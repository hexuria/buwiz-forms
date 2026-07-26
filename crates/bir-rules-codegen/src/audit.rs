use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::error::{CodegenError, Result};
use crate::files::{json_files, read_bytes};
use crate::hash::{digest_entries, sha256_hex};
use crate::json::{JsonValue, canonical_bytes, parse_strict, parse_typed};
use crate::model::{
    BranchState, DerivedInstanceSelector, EvaluationScope, IndexDocument, IndexSnapshot,
    LegacyArtifact, ProfileStates, ReviewStatus, RuleSetDocument, SerializationArtifactBranch,
    SerializationArtifactTarget, SerializationDecimalSeparator, SerializationGrouping,
    SerializationKeyProjection, SerializationNode, SerializationOccurrenceProjection,
    SerializationPresence, SerializationPresentFormat, SerializationValueProjection,
};
use crate::path::{
    DEFAULT_SCHEMA_DIR, DEFAULT_SOURCE_DIR, canonical_repo_root, discover_repo_root,
    normalized_relative_path, portable_join, resolve_existing_under,
};
use crate::schema::SchemaSet;

#[derive(Clone, Debug)]
pub struct AuditOptions {
    pub repo_root: PathBuf,
    pub source_dir: String,
    pub schema_dir: String,
}

impl AuditOptions {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            source_dir: DEFAULT_SOURCE_DIR.to_owned(),
            schema_dir: DEFAULT_SCHEMA_DIR.to_owned(),
        }
    }
}

pub fn discover_default_repo_root() -> Result<PathBuf> {
    let current = std::env::current_dir()
        .map_err(|source| CodegenError::with_source("read current directory", source))?;
    discover_repo_root(&current)
}

/// Opaque result of a complete v2 corpus audit.
///
/// Callers may inspect only immutable summary values. The audited snapshots are
/// deliberately inaccessible so a post-audit identity or review-status
/// mutation cannot be handed to the generator.
///
/// ```compile_fail
/// use bir_rules_codegen::{AuditOptions, audit};
///
/// let mut report = audit(&AuditOptions::new(".")).unwrap();
/// report.snapshots[0]
///     .document
///     .identity
///     .form_code
///     .clear();
/// ```
#[derive(Clone, Debug)]
pub struct AuditReport {
    pub(crate) repo_root: PathBuf,
    pub(crate) schema_digest: String,
    pub(crate) normalized_source_digest: String,
    pub(crate) snapshots: Vec<AuditedSnapshot>,
}

/// Read-only identity and review metadata for one fully audited snapshot.
///
/// The summary deliberately owns no mutable access to the audited document,
/// fixtures, or canonical source bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotSummary {
    rule_set_id: String,
    form_code: String,
    form_revision: String,
    official_package_version: String,
    source_set_sha256: Option<String>,
    review_status: ReviewStatus,
}

impl SnapshotSummary {
    pub fn rule_set_id(&self) -> &str {
        &self.rule_set_id
    }

    pub fn form_code(&self) -> &str {
        &self.form_code
    }

    pub fn form_revision(&self) -> &str {
        &self.form_revision
    }

    pub fn official_package_version(&self) -> &str {
        &self.official_package_version
    }

    pub fn source_set_sha256(&self) -> Option<&str> {
        self.source_set_sha256.as_deref()
    }

    pub fn review_status(&self) -> &'static str {
        match self.review_status {
            ReviewStatus::Skeleton => "skeleton",
            ReviewStatus::Candidate => "candidate",
            ReviewStatus::Reviewed => "reviewed",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AuditedSnapshot {
    pub(crate) index: IndexSnapshot,
    pub(crate) document: RuleSetDocument,
    pub(crate) canonical_rule_set: Vec<u8>,
    pub(crate) serialization_contract_sha256: String,
    pub(crate) normalized_source_sha256: String,
    pub(crate) fixtures: BTreeMap<String, JsonValue>,
}

impl AuditReport {
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    pub fn schema_digest(&self) -> &str {
        &self.schema_digest
    }

    pub fn normalized_source_digest(&self) -> &str {
        &self.normalized_source_digest
    }

    /// Requires an exact rule-set identity without narrowing the audited
    /// aggregate or exposing its mutable internals.
    pub fn require_rule_set(&self, rule_set_id: &str) -> Result<SnapshotSummary> {
        let snapshot = self
            .snapshots
            .iter()
            .find(|snapshot| snapshot.index.rule_set_id == rule_set_id)
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "v2 audit contains no snapshot with rule_set_id `{rule_set_id}`"
                ))
            })?;
        Ok(SnapshotSummary {
            rule_set_id: snapshot.index.rule_set_id.clone(),
            form_code: snapshot.index.form_code.clone(),
            form_revision: snapshot.index.form_revision.clone(),
            official_package_version: snapshot.index.official_package_version.clone(),
            source_set_sha256: snapshot.index.source_set_sha256.clone(),
            review_status: snapshot.index.review_status,
        })
    }
}

pub fn audit(options: &AuditOptions) -> Result<AuditReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let rules_root = resolve_existing_under(&repo_root, "rules", "rules root")?;
    let source_root =
        resolve_existing_under(&repo_root, &options.source_dir, "v2 source directory")?;
    let schema_root =
        resolve_existing_under(&repo_root, &options.schema_dir, "v2 schema directory")?;
    let schemas = SchemaSet::load(&schema_root)?;
    let schema_digest =
        digest_owned_entries("bir-rules-schema-set-v1", schemas.canonical_documents());

    let index_path = source_root.join("index.json");
    let index_bytes = read_bytes(&index_path)?;
    let (index, index_json) = parse_typed::<IndexDocument>(&index_bytes, &index_path)?;
    schemas.validate("index.schema.json", &index_json)?;

    let all_json_files = json_files(&source_root)?;
    let mut canonical_source_files = BTreeMap::new();
    for path in &all_json_files {
        let relative = normalized_relative_path(&source_root, path)?;
        let value = parse_strict(&read_bytes(path)?, path)?;
        canonical_source_files.insert(relative, canonical_bytes(&value));
    }

    let indexed_paths = validate_index(&index, &source_root, &all_json_files)?;
    let source_prefix = normalized_relative_path(&rules_root, &source_root)?;
    let mut snapshots = Vec::with_capacity(index.snapshots.len());
    let mut referenced_fixtures = BTreeSet::new();

    for entry in &index.snapshots {
        let rule_set_path = portable_join(&source_root, &entry.path, "snapshot path")?;
        let rule_set_bytes = read_bytes(&rule_set_path)?;
        let (document, rule_set_json) =
            parse_typed::<RuleSetDocument>(&rule_set_bytes, &rule_set_path)?;
        schemas.validate("rule-set.schema.json", &rule_set_json)?;

        let mut fixture_values = BTreeMap::new();
        for fixture in &document.fixtures {
            let fixture_path = resolve_existing_under(&rules_root, fixture, "v2 fixture path")?;
            let expected_prefix = format!("{source_prefix}/{}/fixtures/", entry.rule_set_id);
            if !fixture.starts_with(&expected_prefix) || !fixture.ends_with(".json") {
                return Err(CodegenError::new(format!(
                    "snapshot `{}` fixture `{fixture}` must be under `{expected_prefix}`",
                    entry.rule_set_id
                )));
            }
            let source_relative = normalized_relative_path(&source_root, &fixture_path)?;
            if !referenced_fixtures.insert(source_relative.clone()) {
                return Err(CodegenError::new(format!(
                    "fixture `{fixture}` is referenced more than once"
                )));
            }
            let fixture_json = parse_strict(&read_bytes(&fixture_path)?, &fixture_path)?;
            schemas.validate("fixture.schema.json", &fixture_json)?;
            fixture_values.insert(fixture.clone(), fixture_json);
        }

        validate_snapshot(
            entry,
            &document,
            &rule_set_json,
            &fixture_values,
            &rules_root,
        )?;

        let normalized_source_sha256 = snapshot_source_digest(&rule_set_json, &fixture_values)?;
        let serialization_contract_sha256 =
            serialization_contract_digest(&rule_set_json).map_err(|error| {
                CodegenError::new(format!(
                    "snapshot `{}` serialization digest failed: {}",
                    entry.rule_set_id,
                    error.message()
                ))
            })?;
        if let Some(expected) = &document.identity.source_set_sha256 {
            if expected != &normalized_source_sha256 {
                return Err(CodegenError::new(format!(
                    "snapshot `{}` pins source_set_sha256 `{expected}`, but normalized source is `{normalized_source_sha256}`",
                    entry.rule_set_id
                )));
            }
        }

        snapshots.push(AuditedSnapshot {
            index: entry.clone(),
            document,
            canonical_rule_set: canonical_bytes(&rule_set_json),
            serialization_contract_sha256,
            normalized_source_sha256,
            fixtures: fixture_values,
        });
    }

    let actual_fixtures = all_json_files
        .iter()
        .filter_map(|path| {
            let relative = normalized_relative_path(&source_root, path).ok()?;
            (relative != "index.json" && !indexed_paths.contains(&relative)).then_some(relative)
        })
        .collect::<BTreeSet<_>>();
    if actual_fixtures != referenced_fixtures {
        return Err(ordered_set_mismatch(
            "fixture file/list bijection",
            &referenced_fixtures,
            &actual_fixtures,
        ));
    }

    let normalized_source_digest =
        digest_owned_entries("bir-rules-normalized-source-v1", &canonical_source_files);

    Ok(AuditReport {
        repo_root,
        schema_digest,
        normalized_source_digest,
        snapshots,
    })
}

#[cfg(test)]
mod snapshot_summary_tests {
    use super::{AuditOptions, audit};

    const RULE_SET_ID: &str = "2550q-v2024-p7.9.6.0";

    #[test]
    fn required_rule_set_exposes_only_immutable_audited_summary() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = audit(&AuditOptions::new(root)).expect("audit landed v2 corpus");

        let summary = report
            .require_rule_set(RULE_SET_ID)
            .expect("require landed candidate");
        assert_eq!(summary.rule_set_id(), RULE_SET_ID);
        assert_eq!(summary.form_code(), "2550Q");
        assert_eq!(summary.form_revision(), "2024-04-01");
        assert_eq!(summary.official_package_version(), "7.9.6.0");
        assert_eq!(summary.review_status(), "candidate");
        assert_eq!(
            summary
                .source_set_sha256()
                .expect("candidate source-set digest")
                .len(),
            64
        );
        assert_eq!(report.snapshot_count(), 1, "focus must not narrow audit");
    }

    #[test]
    fn unknown_required_rule_set_fails_closed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = audit(&AuditOptions::new(root)).expect("audit landed v2 corpus");

        let error = report
            .require_rule_set("unknown-rule-set")
            .expect_err("unknown selector must fail");
        assert!(
            error
                .message()
                .contains("no snapshot with rule_set_id `unknown-rule-set`")
        );
        assert_eq!(
            report.snapshot_count(),
            1,
            "failed focus must not narrow audit"
        );
    }
}

fn validate_index(
    index: &IndexDocument,
    source_root: &Path,
    files: &[PathBuf],
) -> Result<BTreeSet<String>> {
    if index.schema_version != "2.0.0" {
        return Err(CodegenError::new(format!(
            "index schema_version must be `2.0.0`, found `{}`",
            index.schema_version
        )));
    }
    if !index.schema.ends_with("index.schema.json") {
        return Err(CodegenError::new(format!(
            "index $schema `{}` does not identify index.schema.json",
            index.schema
        )));
    }

    let mut ids = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut indexed_paths = BTreeSet::new();
    let mut previous_id: Option<&str> = None;
    for snapshot in &index.snapshots {
        if !ids.insert(snapshot.rule_set_id.clone()) {
            return Err(CodegenError::new(format!(
                "duplicate index rule_set_id `{}`",
                snapshot.rule_set_id
            )));
        }
        let identity = (
            snapshot.form_code.clone(),
            snapshot.form_revision.clone(),
            snapshot.official_package_version.clone(),
        );
        if !identities.insert(identity) {
            return Err(CodegenError::new(format!(
                "duplicate form/revision/package identity for `{}`",
                snapshot.rule_set_id
            )));
        }
        if previous_id.is_some_and(|previous| previous >= snapshot.rule_set_id.as_str()) {
            return Err(CodegenError::new(
                "index snapshots must be strictly ordered by rule_set_id",
            ));
        }
        previous_id = Some(&snapshot.rule_set_id);

        let expected_path = format!("{}/rule-set.json", snapshot.rule_set_id);
        if snapshot.path != expected_path {
            return Err(CodegenError::new(format!(
                "index path for `{}` must be `{expected_path}`, found `{}`",
                snapshot.rule_set_id, snapshot.path
            )));
        }
        portable_join(source_root, &snapshot.path, "index snapshot path")?;
        indexed_paths.insert(snapshot.path.clone());
    }

    let actual_paths = files
        .iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("rule-set.json"))
        .map(|path| normalized_relative_path(source_root, path))
        .collect::<Result<BTreeSet<_>>>()?;
    if actual_paths != indexed_paths {
        return Err(set_mismatch(
            "index/rule-set directory bijection",
            &indexed_paths,
            &actual_paths,
        ));
    }
    Ok(indexed_paths)
}

fn validate_snapshot(
    index: &IndexSnapshot,
    document: &RuleSetDocument,
    rule_set_json: &JsonValue,
    fixtures: &BTreeMap<String, JsonValue>,
    rules_root: &Path,
) -> Result<()> {
    if document.schema_version != "2.0.0" {
        return Err(CodegenError::new(format!(
            "snapshot `{}` schema_version must be `2.0.0`",
            index.rule_set_id
        )));
    }
    if !document.schema.ends_with("rule-set.schema.json") {
        return Err(CodegenError::new(format!(
            "snapshot `{}` $schema does not identify rule-set.schema.json",
            index.rule_set_id
        )));
    }

    let identity = &document.identity;
    compare_identity("rule_set_id", &index.rule_set_id, &identity.rule_set_id)?;
    compare_identity("form_code", &index.form_code, &identity.form_code)?;
    compare_identity(
        "form_revision",
        &index.form_revision,
        &identity.form_revision,
    )?;
    compare_identity(
        "official_package_version",
        &index.official_package_version,
        &identity.official_package_version,
    )?;
    if index.source_set_sha256 != identity.source_set_sha256 {
        return Err(CodegenError::new(format!(
            "snapshot `{}` source_set_sha256 differs between index and rule set",
            index.rule_set_id
        )));
    }
    if index.review_status != document.review_status {
        return Err(CodegenError::new(format!(
            "snapshot `{}` review_status differs between index and rule set",
            index.rule_set_id
        )));
    }
    let document_states = ProfileStates {
        official: document.profile_status.official.state(),
        filing_safe: document.profile_status.filing_safe.state(),
    };
    if index.profile_states != document_states {
        return Err(CodegenError::new(format!(
            "snapshot `{}` profile states differ between index and rule set",
            index.rule_set_id
        )));
    }

    if document.review_status == ReviewStatus::Reviewed {
        if document.identity.source_set_sha256.is_none()
            || document_states.official != BranchState::Executable
            || document_states.filing_safe != BranchState::Executable
        {
            return Err(CodegenError::new(format!(
                "reviewed snapshot `{}` must pin its digest and make both profiles executable",
                index.rule_set_id
            )));
        }
        validate_reviewed_evaluation_policy(document)?;
        if contains_state(rule_set_json, "unresolved") {
            return Err(CodegenError::new(format!(
                "reviewed snapshot `{}` contains unresolved content",
                index.rule_set_id
            )));
        }
    }
    if document.review_status == ReviewStatus::Candidate {
        validate_candidate_readiness(document, fixtures)?;
    }

    reject_machine_local_paths(rule_set_json, "$")?;
    validate_sources(document, rules_root)?;
    validate_boolean_coercion_tokens(document)?;
    validate_legacy_mapping(document, rules_root)?;
    validate_references(document, rule_set_json, fixtures)?;
    validate_scoped_evaluation_contract(document)?;
    validate_serialization_contract(document)?;
    validate_calculation_graph(document)?;
    validate_field_coverage(document)?;
    validate_rule_order(document)?;
    validate_fixture_identity(index, document, fixtures)?;
    if document.review_status == ReviewStatus::Reviewed {
        validate_reviewed_completeness(document, fixtures, rules_root)?;
    }
    Ok(())
}

fn compare_identity(label: &str, index: &str, document: &str) -> Result<()> {
    if index == document {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "snapshot {label} differs between index `{index}` and rule set `{document}`"
        )))
    }
}

fn validate_sources(document: &RuleSetDocument, rules_root: &Path) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut physical_sources = BTreeMap::new();
    for source in &document.sources {
        if !ids.insert(source.source_id.clone()) {
            return Err(CodegenError::new(format!(
                "duplicate source_id `{}`",
                source.source_id
            )));
        }
        let path = resolve_existing_under(rules_root, &source.path, "source artifact path")?;
        let normalized_path = normalized_relative_path(rules_root, &path)?;
        let physical_identity = (normalized_path.clone(), source.sha256.clone());
        if let Some(existing_id) =
            physical_sources.insert(physical_identity, source.source_id.as_str())
        {
            return Err(CodegenError::new(format!(
                "source `{}` aliases physical evidence `{normalized_path}` already named by source `{existing_id}` with sha256 {}",
                source.source_id, source.sha256
            )));
        }
        let actual = sha256_hex(&read_bytes(&path)?);
        if actual != source.sha256 {
            return Err(CodegenError::new(format!(
                "source `{}` hash mismatch for `{}`: expected {}, found {actual}",
                source.source_id, source.path, source.sha256
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_boolean_coercion_tokens(document: &RuleSetDocument) -> Result<()> {
    for (field_index, field) in document.fields.iter().enumerate() {
        let field_path = format!("$.fields[{field_index}]");
        let field = required_object(field, "field")?;
        let field_id = required_string(field, "field_id", "field")?;
        let behavior = required_object(
            field
                .get("behavior")
                .ok_or_else(|| CodegenError::new(format!("{field_path}: missing behavior")))?,
            "field behavior",
        )?;
        for profile in ["official", "filing_safe"] {
            let branch_path = format!("{field_path}.behavior.{profile}");
            let branch = required_object(
                behavior.get(profile).ok_or_else(|| {
                    CodegenError::new(format!("{branch_path}: missing behavior branch"))
                })?,
                "field behavior branch",
            )?;
            if branch.get("state").and_then(JsonValue::as_str) != Some("executable") {
                continue;
            }
            let coercion = required_object(
                branch.get("coercion").ok_or_else(|| {
                    CodegenError::new(format!("{branch_path}: missing executable coercion"))
                })?,
                "field coercion",
            )?;
            if coercion.get("kind").and_then(JsonValue::as_str) != Some("boolean") {
                continue;
            }

            let true_values = required_array(coercion, "true_values", "boolean coercion")?
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    value.as_str().ok_or_else(|| {
                        CodegenError::new(format!(
                            "{branch_path}.coercion.true_values[{index}]: expected string"
                        ))
                    })
                })
                .collect::<Result<BTreeSet<_>>>()?;
            for (index, value) in required_array(coercion, "false_values", "boolean coercion")?
                .iter()
                .enumerate()
            {
                let value = value.as_str().ok_or_else(|| {
                    CodegenError::new(format!(
                        "{branch_path}.coercion.false_values[{index}]: expected string"
                    ))
                })?;
                if true_values.contains(value) {
                    return Err(CodegenError::new(format!(
                        "{branch_path}.coercion: boolean coercion for field `{field_id}` maps {value:?} as both true and false"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_legacy_mapping(document: &RuleSetDocument, rules_root: &Path) -> Result<()> {
    if document.legacy_v1.schema_version != "1.0.0" {
        return Err(CodegenError::new(
            "legacy_v1 schema_version must be `1.0.0`",
        ));
    }
    let expected_artifacts = BTreeSet::from([
        LegacyArtifact::Manifest,
        LegacyArtifact::Fields,
        LegacyArtifact::Validations,
        LegacyArtifact::Calculations,
        LegacyArtifact::Workflow,
    ]);
    let actual_artifacts = document
        .legacy_v1
        .mappings
        .iter()
        .map(|mapping| mapping.artifact.clone())
        .collect::<BTreeSet<_>>();
    if expected_artifacts != actual_artifacts {
        return Err(CodegenError::new(
            "legacy_v1 mappings must cover each of manifest, fields, validations, calculations, and workflow exactly once",
        ));
    }

    let sources = document
        .sources
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut legacy_documents = BTreeMap::new();
    for mapping in &document.legacy_v1.mappings {
        let source = sources.get(mapping.source_id.as_str()).ok_or_else(|| {
            CodegenError::new(format!(
                "legacy {} mapping references missing source `{}`",
                mapping.artifact.label(),
                mapping.source_id
            ))
        })?;
        if source.kind != mapping.artifact.expected_source_kind() {
            return Err(CodegenError::new(format!(
                "legacy {} mapping source `{}` has kind `{}`, expected `{}`",
                mapping.artifact.label(),
                mapping.source_id,
                source.kind,
                mapping.artifact.expected_source_kind()
            )));
        }
        let path = resolve_existing_under(rules_root, &source.path, "legacy source path")?;
        let value = parse_strict(&read_bytes(&path)?, &path)?;
        let actual_count = match mapping.artifact {
            LegacyArtifact::Manifest => 1,
            LegacyArtifact::Fields => array_len_property(&value, "fields")?,
            LegacyArtifact::Validations => array_len_property(&value, "rules")?,
            LegacyArtifact::Calculations => array_len_property(&value, "calculations")?,
            // `legacy_v1.schema_version = 1.0.0` defined this mapping count as
            // the transition count. Keep that wire-compatible for the landed
            // 2550Q candidate; the complete workflow record universe is
            // validated and reconciled from both declared counts below.
            LegacyArtifact::Workflow => array_len_property(&value, "transitions")?,
        };
        if actual_count != mapping.record_count {
            return Err(CodegenError::new(format!(
                "legacy {} mapping declares {} records, source has {actual_count}",
                mapping.artifact.label(),
                mapping.record_count
            )));
        }
        legacy_documents.insert(mapping.artifact, value);
    }

    let counts = &document.legacy_v1.declared_counts;
    validate_mapping_count(document, LegacyArtifact::Fields, counts.typed_fields)?;
    validate_mapping_count(
        document,
        LegacyArtifact::Validations,
        counts.validation_rules,
    )?;
    validate_mapping_count(document, LegacyArtifact::Calculations, counts.calculations)?;
    validate_mapping_count(
        document,
        LegacyArtifact::Workflow,
        counts.workflow_transitions,
    )?;
    let workflow = legacy_documents
        .get(&LegacyArtifact::Workflow)
        .expect("legacy workflow source coverage was validated");
    validate_legacy_workflow_count(
        "states",
        array_len_property(workflow, "phases")?,
        counts.workflow_states,
    )?;
    validate_legacy_workflow_count(
        "transitions",
        array_len_property(workflow, "transitions")?,
        counts.workflow_transitions,
    )?;
    validate_legacy_record_classifications(document, &legacy_documents)?;
    Ok(())
}

fn validate_legacy_workflow_count(label: &str, actual: u64, declared: u64) -> Result<()> {
    if actual == declared {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "legacy workflow {label} source count {actual} differs from declared count {declared}"
        )))
    }
}

fn validate_legacy_record_classifications(
    document: &RuleSetDocument,
    legacy_documents: &BTreeMap<LegacyArtifact, JsonValue>,
) -> Result<()> {
    let source_ids = document
        .legacy_v1
        .mappings
        .iter()
        .map(|mapping| (mapping.artifact, mapping.source_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let represented = represented_legacy_locators(document, &source_ids, legacy_documents)?;
    let mut classified = BTreeSet::new();

    for classification in &document.legacy_v1.record_classifications {
        let artifact = classification.artifact();
        let allowed_array_keys: &[&'static str] = match artifact {
            LegacyArtifact::Fields => &["fields"],
            LegacyArtifact::Validations => &["rules"],
            LegacyArtifact::Calculations => &["calculations"],
            LegacyArtifact::Workflow => &["phases", "transitions"],
            LegacyArtifact::Manifest => {
                return Err(CodegenError::new(
                    "legacy manifest cannot be a record-level classification",
                ));
            }
        };
        let locator = classification.locator();
        let (array_key, index) = parse_canonical_legacy_record_locator(
            artifact,
            allowed_array_keys,
            locator,
            "classification",
        )?;
        let key = (artifact, locator.to_owned());
        if !classified.insert(key.clone()) {
            return Err(CodegenError::new(format!(
                "legacy {} record `{locator}` is classified more than once",
                artifact.label()
            )));
        }
        if represented.contains(&key) {
            return Err(CodegenError::new(format!(
                "legacy {} record `{locator}` is both represented by v2 and classified without a runtime target",
                artifact.label()
            )));
        }

        let legacy = legacy_documents.get(&artifact).ok_or_else(|| {
            CodegenError::new(format!(
                "legacy {} source is unavailable for record classification",
                artifact.label()
            ))
        })?;
        let legacy = required_object(legacy, "legacy record classification source")?;
        let records = required_array(legacy, array_key, "legacy record classification source")?;
        let record = records.get(index).ok_or_else(|| {
            CodegenError::new(format!(
                "legacy {} classification locator `{locator}` is out of range",
                artifact.label()
            ))
        })?;
        let record = required_object(record, "legacy classified record")?;
        match artifact {
            LegacyArtifact::Fields | LegacyArtifact::Validations | LegacyArtifact::Calculations => {
                let id_key = match artifact {
                    LegacyArtifact::Fields => "field_key",
                    LegacyArtifact::Validations => "rule_id",
                    LegacyArtifact::Calculations => "calculation_id",
                    LegacyArtifact::Manifest | LegacyArtifact::Workflow => unreachable!(),
                };
                let legacy_id = classification.legacy_id().ok_or_else(|| {
                    CodegenError::new(format!(
                        "legacy {} classification `{locator}` must name its source `{id_key}`",
                        artifact.label()
                    ))
                })?;
                let actual_id = required_string(record, id_key, "legacy classified record")?;
                if actual_id != legacy_id {
                    return Err(CodegenError::new(format!(
                        "legacy {} classification `{locator}` names `{legacy_id}`, source record names `{actual_id}`",
                        artifact.label()
                    )));
                }
            }
            LegacyArtifact::Workflow => {
                if let Some(legacy_id) = classification.legacy_id() {
                    return Err(CodegenError::new(format!(
                        "legacy workflow classification `{locator}` must use locator identity, not invented legacy_id `{legacy_id}`"
                    )));
                }
            }
            LegacyArtifact::Manifest => unreachable!(),
        }
        let expected_source_id = source_ids
            .get(&artifact)
            .expect("legacy source coverage was validated");
        if !classification.source_refs().iter().any(|source_ref| {
            source_ref.source_id.as_str() == *expected_source_id
                && source_ref.locator.as_deref() == Some(locator)
        }) {
            return Err(CodegenError::new(format!(
                "legacy {} classification `{locator}` must cite exact source `{expected_source_id}` and locator",
                artifact.label()
            )));
        }
    }
    Ok(())
}

fn represented_legacy_locators(
    document: &RuleSetDocument,
    source_ids: &BTreeMap<LegacyArtifact, &str>,
    legacy_documents: &BTreeMap<LegacyArtifact, JsonValue>,
) -> Result<BTreeSet<(LegacyArtifact, String)>> {
    let mut represented = BTreeSet::new();
    collect_represented_legacy_entity_locators(
        LegacyArtifact::Fields,
        "fields",
        document.field_groups.iter().chain(document.fields.iter()),
        source_ids,
        legacy_documents,
        &mut represented,
    )?;
    collect_represented_legacy_entity_locators(
        LegacyArtifact::Validations,
        "rules",
        document.rules.iter(),
        source_ids,
        legacy_documents,
        &mut represented,
    )?;
    collect_represented_legacy_entity_locators(
        LegacyArtifact::Calculations,
        "calculations",
        document.calculations.iter(),
        source_ids,
        legacy_documents,
        &mut represented,
    )?;
    if let Some(workflow) = document.workflow.object() {
        if let Some(JsonValue::Array(states)) = workflow.get("states") {
            collect_represented_legacy_entity_locators(
                LegacyArtifact::Workflow,
                "phases",
                states.iter(),
                source_ids,
                legacy_documents,
                &mut represented,
            )?;
        }
        if let Some(JsonValue::Array(transitions)) = workflow.get("transitions") {
            collect_represented_legacy_entity_locators(
                LegacyArtifact::Workflow,
                "transitions",
                transitions.iter(),
                source_ids,
                legacy_documents,
                &mut represented,
            )?;
        }
    }
    Ok(represented)
}

fn collect_represented_legacy_entity_locators<'a>(
    artifact: LegacyArtifact,
    legacy_array_key: &'static str,
    entities: impl Iterator<Item = &'a JsonValue>,
    source_ids: &BTreeMap<LegacyArtifact, &str>,
    legacy_documents: &BTreeMap<LegacyArtifact, JsonValue>,
    represented: &mut BTreeSet<(LegacyArtifact, String)>,
) -> Result<()> {
    for entity in entities {
        let Some(source_refs) = entity
            .object()
            .and_then(|entity| entity.get("source_refs"))
            .and_then(|source_refs| match source_refs {
                JsonValue::Array(source_refs) => Some(source_refs),
                _ => None,
            })
        else {
            continue;
        };
        for source_ref in source_refs {
            let Some(source_ref) = source_ref.object() else {
                continue;
            };
            let Some(source_id) = source_ref.get("source_id").and_then(JsonValue::as_str) else {
                continue;
            };
            let Some(source_artifact) =
                source_ids
                    .iter()
                    .find_map(|(mapped_artifact, mapped_source_id)| {
                        (*mapped_source_id == source_id).then_some(*mapped_artifact)
                    })
            else {
                continue;
            };
            if source_artifact != artifact {
                // A v2 entity may cite a different legacy artifact as
                // supporting evidence (for example, a calculation can cite
                // the validation handler that invokes it). Such a citation is
                // not reconciliation authority for this entity. Ignore it
                // here; without a same-artifact locator the source record
                // remains unrepresented and the aggregate gap stays open.
                continue;
            }
            let locator = source_ref
                .get("locator")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| {
                    CodegenError::new(format!(
                        "v2 {} entity citation of legacy source `{source_id}` must include a record locator",
                        artifact.label()
                    ))
                })?;
            validate_represented_legacy_locator(
                artifact,
                legacy_array_key,
                locator,
                legacy_documents,
            )?;
            if !represented.insert((artifact, locator.to_owned())) {
                return Err(CodegenError::new(format!(
                    "v2 {} source locator `{locator}` is referenced more than once",
                    artifact.label()
                )));
            }
        }
    }
    Ok(())
}

fn validate_represented_legacy_locator(
    artifact: LegacyArtifact,
    array_key: &'static str,
    locator: &str,
    legacy_documents: &BTreeMap<LegacyArtifact, JsonValue>,
) -> Result<()> {
    let (_, index) =
        parse_canonical_legacy_record_locator(artifact, &[array_key], locator, "v2 source")?;
    let legacy = legacy_documents.get(&artifact).ok_or_else(|| {
        CodegenError::new(format!(
            "legacy {} source is unavailable for v2 record reconciliation",
            artifact.label()
        ))
    })?;
    let legacy = required_object(legacy, "legacy represented-record source")?;
    let records = required_array(legacy, array_key, "legacy represented-record source")?;
    if index >= records.len() {
        return Err(CodegenError::new(format!(
            "v2 {} source locator `{locator}` is out of range for {} legacy record(s)",
            artifact.label(),
            records.len()
        )));
    }
    Ok(())
}

fn parse_canonical_legacy_record_locator(
    artifact: LegacyArtifact,
    allowed_array_keys: &[&'static str],
    locator: &str,
    owner: &str,
) -> Result<(&'static str, usize)> {
    let Some((array_key, raw_index)) = allowed_array_keys.iter().find_map(|array_key| {
        locator
            .strip_prefix(&format!("#/{array_key}/"))
            .map(|raw_index| (*array_key, raw_index))
    }) else {
        let expected = allowed_array_keys
            .iter()
            .map(|array_key| format!("#/{array_key}/<index>"))
            .collect::<Vec<_>>()
            .join(" or ");
        return Err(CodegenError::new(format!(
            "legacy {} {owner} locator `{locator}` is invalid for the artifact; expected {expected}",
            artifact.label()
        )));
    };
    let index = raw_index.parse::<usize>().map_err(|_| {
        CodegenError::new(format!(
            "legacy {} {owner} locator `{locator}` must end in a decimal record index",
            artifact.label()
        ))
    })?;
    if raw_index != index.to_string() {
        return Err(CodegenError::new(format!(
            "legacy {} {owner} locator `{locator}` is not a canonical JSON-array locator",
            artifact.label()
        )));
    }
    Ok((array_key, index))
}

fn validate_mapping_count(
    document: &RuleSetDocument,
    artifact: LegacyArtifact,
    expected: u64,
) -> Result<()> {
    let mapping = document
        .legacy_v1
        .mappings
        .iter()
        .find(|mapping| mapping.artifact == artifact)
        .expect("artifact coverage was checked");
    if mapping.record_count == expected {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "legacy {} record_count {} differs from declared count {expected}",
            artifact.label(),
            mapping.record_count
        )))
    }
}

fn validate_references(
    document: &RuleSetDocument,
    rule_set: &JsonValue,
    fixtures: &BTreeMap<String, JsonValue>,
) -> Result<()> {
    let source_ids = document
        .sources
        .iter()
        .map(|source| source.source_id.as_str())
        .collect::<BTreeSet<_>>();
    validate_string_references(rule_set, "source_id", &source_ids, "source", "$")?;
    for (path, fixture) in fixtures {
        validate_string_references(
            fixture,
            "source_id",
            &source_ids,
            "source",
            &format!("fixture:{path}"),
        )?;
    }

    let field_ids = collect_object_ids(&document.fields, "field_id")?;
    let group_ids = collect_object_ids(&document.field_groups, "group_id")?;
    let calculation_ids = collect_object_ids(&document.calculations, "calculation_id")?;
    let context_ids = collect_object_ids(&document.context_values, "context_value_id")?;
    let rule_ids = collect_object_ids(&document.rules, "rule_id")?;

    validate_string_references(rule_set, "field_id", &field_ids, "field", "$")?;
    validate_string_references(rule_set, "group_id", &group_ids, "field group", "$")?;
    validate_string_references(
        rule_set,
        "calculation_id",
        &calculation_ids,
        "calculation",
        "$",
    )?;
    validate_string_references(
        rule_set,
        "context_value_id",
        &context_ids,
        "context value",
        "$",
    )?;
    validate_array_references(&document.rules, "field_ids", &field_ids, "field", "$.rules")?;

    for (path, fixture) in fixtures {
        validate_string_references(
            fixture,
            "field_id",
            &field_ids,
            "field",
            &format!("fixture:{path}"),
        )?;
        validate_string_references(
            fixture,
            "context_value_id",
            &context_ids,
            "context value",
            &format!("fixture:{path}"),
        )?;
        validate_string_references(
            fixture,
            "group_id",
            &group_ids,
            "field group",
            &format!("fixture:{path}"),
        )?;
        validate_optional_string_references(
            fixture,
            "rule_id",
            &rule_ids,
            "rule",
            &format!("fixture:{path}"),
        )?;
        validate_string_references(
            fixture,
            "calculation_id",
            &calculation_ids,
            "calculation",
            &format!("fixture:{path}"),
        )?;
    }

    validate_workflow_references(&document.workflow)?;
    validate_derived_outputs(document, rule_set)?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum SerializationProfile {
    Official,
    FilingSafe,
}

impl SerializationProfile {
    const fn label(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::FilingSafe => "filing_safe",
        }
    }
}

#[derive(Clone, Debug)]
struct SerializationGroupScope<'a> {
    group_id: &'a str,
    min_occurs: usize,
    max_occurs: Option<usize>,
}

#[derive(Clone, Debug)]
enum SerializationKeyLanguage {
    Exact(String),
    Indexed {
        group_id: String,
        index_base: u32,
        index_step: u32,
        padding: u32,
        prefix: String,
        suffix: String,
        max_occurs: Option<usize>,
    },
}

impl SerializationKeyLanguage {
    fn identity(&self) -> String {
        match self {
            Self::Exact(key) => format!("exact:{key}"),
            Self::Indexed {
                group_id,
                index_base,
                index_step,
                padding,
                prefix,
                suffix,
                ..
            } => format!(
                "indexed:{group_id}:{index_base}:{index_step}:{padding}:{prefix:?}:{suffix:?}"
            ),
        }
    }
}

#[derive(Clone, Debug)]
struct SerializationOccurrenceUse {
    key_language: SerializationKeyLanguage,
    key_label: String,
    start: u32,
    end: Option<u32>,
    step: u32,
    fixed_cardinality: bool,
    always_present: bool,
    path: String,
}

fn validate_serialization_contract(document: &RuleSetDocument) -> Result<()> {
    if document.serialization.contract_version != "1.0.0" {
        return Err(CodegenError::new(format!(
            "serialization contract_version must be `1.0.0`, found `{}`",
            document.serialization.contract_version
        )));
    }

    let mut artifact_ids = BTreeSet::new();
    let mut artifact_identities = BTreeSet::new();
    for (index, artifact) in document.serialization.artifacts.iter().enumerate() {
        let path = format!("$.serialization.artifacts[{index}]");
        if !artifact_ids.insert(artifact.artifact_id.as_str()) {
            return Err(CodegenError::new(format!(
                "{path}: duplicate serialization artifact_id `{}`",
                artifact.artifact_id
            )));
        }
        if !artifact_identities.insert((artifact.target, artifact.variant_id.as_str())) {
            return Err(CodegenError::new(format!(
                "{path}: duplicate serialization artifact identity target={:?}, variant_id=`{}`",
                artifact.target, artifact.variant_id
            )));
        }
        validate_artifact_variant_id_parity(&artifact.variant_id, &path)?;

        for (profile, branch) in [
            (SerializationProfile::Official, &artifact.official),
            (SerializationProfile::FilingSafe, &artifact.filing_safe),
        ] {
            if let SerializationArtifactBranch::Executable { nodes, .. } = branch {
                validate_serialization_plan(
                    document,
                    &artifact.artifact_id,
                    artifact.target,
                    profile,
                    nodes,
                    &format!("{path}.{}", profile.label()),
                )?;
            }
        }
    }
    Ok(())
}

fn validate_artifact_variant_id_parity(value: &str, path: &str) -> Result<()> {
    let valid_boundary = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    if value.is_empty()
        || value.len() > 128
        || !value
            .as_bytes()
            .first()
            .copied()
            .is_some_and(valid_boundary)
        || !value.as_bytes().last().copied().is_some_and(valid_boundary)
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(CodegenError::new(format!(
            "{path}: variant_id `{value}` does not match ArtifactVariantId's closed runtime alphabet"
        )));
    }
    Ok(())
}

fn validate_serialization_plan(
    document: &RuleSetDocument,
    artifact_id: &str,
    target: SerializationArtifactTarget,
    profile: SerializationProfile,
    nodes: &[SerializationNode],
    path: &str,
) -> Result<()> {
    let mut ordinals = Vec::new();
    let mut occurrences = Vec::new();
    validate_serialization_nodes(
        document,
        artifact_id,
        target,
        profile,
        nodes,
        path,
        &mut Vec::new(),
        &mut ordinals,
        &mut occurrences,
    )?;

    for (index, (ordinal, ordinal_path)) in ordinals.iter().enumerate() {
        let expected = u32::try_from(index + 1)
            .map_err(|_| CodegenError::new(format!("{path}: too many artifact nodes")))?;
        if *ordinal != expected {
            return Err(CodegenError::new(format!(
                "{ordinal_path}: serialization ordinals must be unique, artifact-global, source-ordered, and contiguous from 1; expected {expected}, found {ordinal}"
            )));
        }
    }
    validate_serialization_occurrences(&occurrences, path)
}

#[allow(clippy::too_many_arguments)]
fn validate_serialization_nodes<'a>(
    document: &'a RuleSetDocument,
    artifact_id: &str,
    target: SerializationArtifactTarget,
    profile: SerializationProfile,
    nodes: &'a [SerializationNode],
    path: &str,
    scopes: &mut Vec<SerializationGroupScope<'a>>,
    ordinals: &mut Vec<(u32, String)>,
    occurrences: &mut Vec<SerializationOccurrenceUse>,
) -> Result<()> {
    for (index, node) in nodes.iter().enumerate() {
        let node_path = format!("{path}.nodes[{index}]");
        ordinals.push((node.ordinal(), node_path.clone()));
        match node {
            SerializationNode::PseudoXmlField {
                key_projection,
                occurrence_projection,
                value_projection,
                semantic_format,
                presence,
                ..
            } => {
                let value_type = validate_serialization_value_projection(
                    document,
                    target,
                    profile,
                    value_projection,
                    scopes,
                    &format!("{node_path}.value_projection"),
                )?;
                validate_serialization_format(
                    semantic_format,
                    value_type,
                    &format!("{node_path}.semantic_format"),
                )?;
                validate_serialization_presence(
                    document,
                    target,
                    profile,
                    presence,
                    scopes,
                    &format!("{node_path}.presence"),
                )?;
                let (key_language, key_label, key_group) = validate_key_projection(
                    key_projection,
                    scopes,
                    &format!("{node_path}.key_projection"),
                )?;
                let (start, end, step, fixed_cardinality, occurrence_group) =
                    validate_occurrence_projection(
                        occurrence_projection,
                        scopes,
                        &format!("{node_path}.occurrence_projection"),
                    )?;

                if key_group.is_some() && key_group == occurrence_group {
                    return Err(CodegenError::new(format!(
                        "{node_path}: key and occurrence projections must not both index the same group; each generated key starts at occurrence 1"
                    )));
                }
                for scope in scopes.iter().filter(|scope| scope.max_occurs != Some(1)) {
                    if key_group.as_deref() != Some(scope.group_id)
                        && occurrence_group.as_deref() != Some(scope.group_id)
                    {
                        return Err(CodegenError::new(format!(
                            "{node_path}: dynamic group `{}` can repeat but neither key nor occurrence projection accounts for its index",
                            scope.group_id
                        )));
                    }
                }
                occurrences.push(SerializationOccurrenceUse {
                    key_language,
                    key_label,
                    start,
                    end,
                    step,
                    fixed_cardinality,
                    always_present: serialization_node_always_emits(presence, semantic_format),
                    path: node_path,
                });
            }
            SerializationNode::MetadataElement {
                value_projection,
                semantic_format,
                presence,
                ..
            } => {
                let value_type = validate_serialization_value_projection(
                    document,
                    target,
                    profile,
                    value_projection,
                    scopes,
                    &format!("{node_path}.value_projection"),
                )?;
                validate_serialization_format(
                    semantic_format,
                    value_type,
                    &format!("{node_path}.semantic_format"),
                )?;
                validate_serialization_presence(
                    document,
                    target,
                    profile,
                    presence,
                    scopes,
                    &format!("{node_path}.presence"),
                )?;
                if scopes.iter().any(|scope| scope.max_occurs != Some(1)) {
                    return Err(CodegenError::new(format!(
                        "{node_path}: metadata elements inside repeating dynamic groups have no key/occurrence projection and are ambiguous"
                    )));
                }
            }
            SerializationNode::ReviewedLiteral { exact_bytes, .. } => {
                if exact_bytes.is_empty() {
                    return Err(CodegenError::new(format!(
                        "{node_path}: reviewed literal exact_bytes must not be empty"
                    )));
                }
            }
            SerializationNode::DynamicGroup {
                group_id,
                min_occurs,
                max_occurs,
                nodes,
                ..
            } => {
                validate_dynamic_group_nesting(scopes, &node_path)?;
                if scopes.iter().any(|scope| scope.group_id == group_id) {
                    return Err(CodegenError::new(format!(
                        "{node_path}: dynamic group `{group_id}` is nested inside itself"
                    )));
                }
                if max_occurs.is_some_and(|maximum| *min_occurs > maximum) {
                    return Err(CodegenError::new(format!(
                        "{node_path}: dynamic group min_occurs {min_occurs} exceeds max_occurs {}",
                        max_occurs.expect("checked Some")
                    )));
                }
                let (declared_min, declared_max) =
                    serialization_group_bounds(document, group_id, &node_path)?;
                if *min_occurs != declared_min || *max_occurs != declared_max {
                    return Err(CodegenError::new(format!(
                        "{node_path}: dynamic group `{group_id}` bounds ({min_occurs}, {max_occurs:?}) do not exactly match field-group bounds ({declared_min}, {declared_max:?})"
                    )));
                }

                scopes.push(SerializationGroupScope {
                    group_id,
                    min_occurs: *min_occurs,
                    max_occurs: *max_occurs,
                });
                validate_serialization_nodes(
                    document,
                    artifact_id,
                    target,
                    profile,
                    nodes,
                    &node_path,
                    scopes,
                    ordinals,
                    occurrences,
                )?;
                scopes.pop();
            }
        }
    }
    let _ = artifact_id;
    Ok(())
}

fn validate_dynamic_group_nesting(
    scopes: &[SerializationGroupScope<'_>],
    path: &str,
) -> Result<()> {
    if let Some(parent) = scopes.last() {
        Err(CodegenError::new(format!(
            "{path}: nested dynamic groups are unsupported until parent-child repeated-instance identity is represented; parent group is `{}`",
            parent.group_id
        )))
    } else {
        Ok(())
    }
}

fn serialization_node_always_emits(
    presence: &SerializationPresence,
    semantic_format: &crate::model::SerializationSemanticFormat,
) -> bool {
    matches!(presence, SerializationPresence::Always)
        && semantic_format.absent != crate::model::SerializationAbsentPolicy::OmitOccurrence
        && semantic_format.blank != crate::model::SerializationBlankPolicy::OmitOccurrence
}

fn validate_key_projection(
    projection: &SerializationKeyProjection,
    scopes: &[SerializationGroupScope<'_>],
    path: &str,
) -> Result<(SerializationKeyLanguage, String, Option<String>)> {
    match projection {
        SerializationKeyProjection::Exact { key } => Ok((
            SerializationKeyLanguage::Exact(key.clone()),
            format!("exact key `{key}`"),
            None,
        )),
        SerializationKeyProjection::GroupIndexed {
            group_id,
            index_base,
            index_step,
            padding,
            prefix,
            suffix,
            ..
        } => {
            let scope = enclosing_serialization_group(scopes, group_id, path)?;
            if *index_step == 0 {
                return Err(CodegenError::new(format!(
                    "{path}: group-indexed key index_step must be positive"
                )));
            }
            let end =
                validate_bounded_index(*index_base, *index_step, scope.max_occurs, path, "key")?;
            let sample = format!(
                "{prefix}{:0width$}{suffix}",
                index_base,
                width = *padding as usize
            );
            validate_runtime_stable_id(&sample, path, "generated key")?;
            let endpoint = format!("{prefix}{end:0width$}{suffix}", width = *padding as usize);
            validate_runtime_stable_id(&endpoint, path, "generated key endpoint")?;
            Ok((
                SerializationKeyLanguage::Indexed {
                    group_id: group_id.clone(),
                    index_base: *index_base,
                    index_step: *index_step,
                    padding: *padding,
                    prefix: prefix.clone(),
                    suffix: suffix.clone(),
                    max_occurs: scope.max_occurs,
                },
                format!(
                    "indexed key `{prefix}<{}:{index_base}+n*{index_step}>{suffix}`",
                    padding
                ),
                Some(scope.group_id.to_owned()),
            ))
        }
    }
}

fn validate_occurrence_projection(
    projection: &SerializationOccurrenceProjection,
    scopes: &[SerializationGroupScope<'_>],
    path: &str,
) -> Result<(u32, Option<u32>, u32, bool, Option<String>)> {
    match projection {
        SerializationOccurrenceProjection::Fixed { occurrence } => {
            if *occurrence == 0 {
                return Err(CodegenError::new(format!(
                    "{path}: fixed occurrence must be positive"
                )));
            }
            Ok((*occurrence, Some(*occurrence), 1, true, None))
        }
        SerializationOccurrenceProjection::GroupIndexed {
            group_id,
            index_base,
            index_step,
            ..
        } => {
            let scope = enclosing_serialization_group(scopes, group_id, path)?;
            if *index_base == 0 || *index_step == 0 {
                return Err(CodegenError::new(format!(
                    "{path}: group-indexed occurrence base and step must be positive"
                )));
            }
            if *index_step != 1 {
                return Err(CodegenError::new(format!(
                    "{path}: group-indexed occurrence step {index_step} creates gaps; occurrence numbering must be contiguous"
                )));
            }
            let end = validate_bounded_index(
                *index_base,
                *index_step,
                scope.max_occurs,
                path,
                "occurrence",
            )?;
            Ok((
                *index_base,
                Some(end),
                *index_step,
                scope.max_occurs == Some(scope.min_occurs),
                Some(scope.group_id.to_owned()),
            ))
        }
    }
}

fn enclosing_serialization_group<'a, 'group>(
    scopes: &'a [SerializationGroupScope<'group>],
    group_id: &str,
    path: &str,
) -> Result<&'a SerializationGroupScope<'group>> {
    scopes
        .iter()
        .rev()
        .find(|scope| scope.group_id == group_id)
        .ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: indexed projection references group `{group_id}` outside its dynamic-group scope"
            ))
        })
}

fn validate_bounded_index(
    base: u32,
    step: u32,
    max_occurs: Option<usize>,
    path: &str,
    label: &str,
) -> Result<u32> {
    let Some(maximum) = max_occurs else {
        return Err(CodegenError::new(format!(
            "{path}: group-indexed {label} projection requires a bounded dynamic group"
        )));
    };
    if maximum == 0 {
        return Ok(base);
    }
    let offset = u64::try_from(maximum - 1)
        .ok()
        .and_then(|count| count.checked_mul(u64::from(step)))
        .and_then(|offset| u64::from(base).checked_add(offset))
        .ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: bounded {label} index projection overflows"
            ))
        })?;
    let end = u32::try_from(offset).map_err(|_| {
        CodegenError::new(format!(
            "{path}: bounded {label} index projection exceeds u32"
        ))
    })?;
    Ok(end)
}

fn validate_runtime_stable_id(value: &str, path: &str, label: &str) -> Result<()> {
    let boundary = |byte: u8| byte.is_ascii_alphanumeric();
    if value.is_empty()
        || value.len() > 255
        || !value.as_bytes().first().copied().is_some_and(boundary)
        || !value.as_bytes().last().copied().is_some_and(boundary)
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
    {
        return Err(CodegenError::new(format!(
            "{path}: {label} `{value}` is outside the closed runtime stable-ID alphabet"
        )));
    }
    Ok(())
}

fn validate_serialization_occurrences(
    occurrences: &[SerializationOccurrenceUse],
    path: &str,
) -> Result<()> {
    for occurrence in occurrences {
        if !occurrence.always_present && occurrence.end.is_none_or(|end| end > occurrence.start) {
            return Err(CodegenError::new(format!(
                "{}: conditional/omitted presence cannot govern a repeated occurrence range because skipping an earlier instance would shift later physical numbering",
                occurrence.path
            )));
        }
    }
    for left_index in 0..occurrences.len() {
        for right in &occurrences[left_index + 1..] {
            let left = &occurrences[left_index];
            if !serialization_key_languages_overlap(&left.key_language, &right.key_language) {
                continue;
            }
            if serialization_occurrence_ranges_overlap(left, right) {
                return Err(CodegenError::new(format!(
                    "{}: overlapping exact/generated key+occurrence projection with `{}`; key languages can produce the same physical key",
                    right.path, left.path
                )));
            }
            if (!left.always_present && left.start < right.start)
                || (!right.always_present && right.start < left.start)
            {
                return Err(CodegenError::new(format!(
                    "{}: conditional/omitted earlier occurrence for an overlapping key language can shift a later physical occurrence",
                    if !left.always_present {
                        &left.path
                    } else {
                        &right.path
                    }
                )));
            }
        }
    }

    let mut by_key = BTreeMap::<String, Vec<&SerializationOccurrenceUse>>::new();
    for occurrence in occurrences {
        by_key
            .entry(occurrence.key_language.identity())
            .or_default()
            .push(occurrence);
    }
    for entries in by_key.values_mut() {
        entries.sort_by(|left, right| {
            (left.start, left.end, left.path.as_str()).cmp(&(
                right.start,
                right.end,
                right.path.as_str(),
            ))
        });
        let label = &entries[0].key_label;
        let mut expected = 1_u32;
        for (index, entry) in entries.iter().enumerate() {
            if entry.start < expected {
                return Err(CodegenError::new(format!(
                    "{}: overlapping exact key+occurrence projection for {label}; occurrence {} was already covered",
                    entry.path, entry.start
                )));
            }
            if entry.start > expected {
                return Err(CodegenError::new(format!(
                    "{}: bad occurrence numbering for {label}; expected occurrence {expected}, found {}",
                    entry.path, entry.start
                )));
            }
            if entry.step != 1 {
                return Err(CodegenError::new(format!(
                    "{}: occurrence step {} is not contiguous for {label}",
                    entry.path, entry.step
                )));
            }
            if index + 1 < entries.len() && !entry.always_present {
                return Err(CodegenError::new(format!(
                    "{}: conditional/omitted occurrence for {label} cannot precede another occurrence because the physical numbering would shift",
                    entry.path
                )));
            }
            match entry.end {
                Some(end) => {
                    expected = end.checked_add(1).ok_or_else(|| {
                        CodegenError::new(format!(
                            "{}: occurrence range overflows for {label}",
                            entry.path
                        ))
                    })?;
                    if index + 1 < entries.len() && !entry.fixed_cardinality {
                        return Err(CodegenError::new(format!(
                            "{}: variable-cardinality occurrence range for {label} cannot be followed by another fixed projection without creating runtime gaps",
                            entry.path
                        )));
                    }
                }
                None if index + 1 < entries.len() => {
                    return Err(CodegenError::new(format!(
                        "{}: unbounded occurrence range for {label} overlaps every later projection",
                        entry.path
                    )));
                }
                None => {}
            }
        }
    }
    let _ = path;
    Ok(())
}

fn serialization_occurrence_ranges_overlap(
    left: &SerializationOccurrenceUse,
    right: &SerializationOccurrenceUse,
) -> bool {
    let left_reaches_right = left.end.is_none_or(|end| right.start <= end);
    let right_reaches_left = right.end.is_none_or(|end| left.start <= end);
    left_reaches_right && right_reaches_left
}

fn serialization_key_languages_overlap(
    left: &SerializationKeyLanguage,
    right: &SerializationKeyLanguage,
) -> bool {
    match (left, right) {
        (SerializationKeyLanguage::Exact(left), SerializationKeyLanguage::Exact(right)) => {
            left == right
        }
        (
            SerializationKeyLanguage::Exact(exact),
            indexed @ SerializationKeyLanguage::Indexed { .. },
        )
        | (
            indexed @ SerializationKeyLanguage::Indexed { .. },
            SerializationKeyLanguage::Exact(exact),
        ) => indexed_key_language_contains(indexed, exact),
        (
            left @ SerializationKeyLanguage::Indexed {
                prefix: left_prefix,
                suffix: left_suffix,
                padding: left_padding,
                ..
            },
            right @ SerializationKeyLanguage::Indexed {
                prefix: right_prefix,
                suffix: right_suffix,
                padding: right_padding,
                ..
            },
        ) => {
            if let (Some(left_keys), Some(right_keys)) = (
                enumerate_indexed_keys(left, 10_000),
                enumerate_indexed_keys(right, 10_000),
            ) {
                let left_keys = left_keys.into_iter().collect::<BTreeSet<_>>();
                return right_keys
                    .into_iter()
                    .any(|candidate| left_keys.contains(&candidate));
            }
            if left_prefix == right_prefix
                && left_suffix == right_suffix
                && left_padding == right_padding
            {
                return indexed_numeric_sequences_may_intersect(left, right);
            }
            // Different envelopes or padding can still converge (for example,
            // padding ceases to matter after enough digits). Without a bounded
            // proof of disjointness, fail closed.
            true
        }
    }
}

fn indexed_key_language_contains(language: &SerializationKeyLanguage, exact: &str) -> bool {
    let SerializationKeyLanguage::Indexed {
        index_base,
        index_step,
        padding,
        prefix,
        suffix,
        max_occurs,
        ..
    } = language
    else {
        return false;
    };
    let Some(middle) = exact
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
    else {
        return false;
    };
    if middle.is_empty() || !middle.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let Ok(value) = middle.parse::<u32>() else {
        return false;
    };
    if format!("{value:0width$}", width = *padding as usize) != middle {
        return false;
    }
    if value < *index_base || (value - *index_base) % *index_step != 0 {
        return false;
    }
    let instance_index = usize::try_from((value - *index_base) / *index_step)
        .expect("u32 always fits supported usize");
    max_occurs.is_none_or(|maximum| instance_index < maximum)
}

fn enumerate_indexed_keys(
    language: &SerializationKeyLanguage,
    limit: usize,
) -> Option<Vec<String>> {
    let SerializationKeyLanguage::Indexed {
        index_base,
        index_step,
        padding,
        prefix,
        suffix,
        max_occurs: Some(maximum),
        ..
    } = language
    else {
        return None;
    };
    if *maximum > limit {
        return None;
    }
    let mut keys = Vec::with_capacity(*maximum);
    for instance_index in 0..*maximum {
        let value = u64::from(*index_base).checked_add(
            u64::try_from(instance_index)
                .ok()?
                .checked_mul(u64::from(*index_step))?,
        )?;
        keys.push(format!(
            "{prefix}{value:0width$}{suffix}",
            width = *padding as usize
        ));
    }
    Some(keys)
}

fn indexed_numeric_sequences_may_intersect(
    left: &SerializationKeyLanguage,
    right: &SerializationKeyLanguage,
) -> bool {
    let SerializationKeyLanguage::Indexed {
        index_base: left_base,
        index_step: left_step,
        max_occurs: left_max,
        ..
    } = left
    else {
        return false;
    };
    let SerializationKeyLanguage::Indexed {
        index_base: right_base,
        index_step: right_step,
        max_occurs: right_max,
        ..
    } = right
    else {
        return false;
    };
    if let Some(keys) = enumerate_indexed_keys(left, 100_000) {
        return keys
            .iter()
            .any(|key| indexed_key_language_contains(right, key));
    }
    if let Some(keys) = enumerate_indexed_keys(right, 100_000) {
        return keys
            .iter()
            .any(|key| indexed_key_language_contains(left, key));
    }
    let difference = i64::from(*left_base) - i64::from(*right_base);
    let divisor = gcd_u64(u64::from(*left_step), u64::from(*right_step));
    if difference.unsigned_abs() % divisor != 0 {
        return false;
    }
    // Both are unbounded or too large to enumerate. Congruent arithmetic
    // progressions have a common value; bounded exhaustion was handled above.
    let _ = (left_max, right_max);
    true
}

fn gcd_u64(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn validate_serialization_value_projection<'a>(
    document: &'a RuleSetDocument,
    target: SerializationArtifactTarget,
    profile: SerializationProfile,
    projection: &'a SerializationValueProjection,
    scopes: &[SerializationGroupScope<'_>],
    path: &str,
) -> Result<&'a str> {
    match projection {
        SerializationValueProjection::Field { field } => {
            validate_serialization_field_ref(document, field, scopes, path, profile)
        }
        SerializationValueProjection::Derived {
            calculation_id,
            output_id,
            instance,
        } => serialization_derived_output_type(
            document,
            target,
            profile,
            calculation_id,
            output_id,
            instance,
            scopes.last().map(|scope| scope.group_id),
            path,
        ),
        SerializationValueProjection::Context { context_value_id } => document
            .context_values
            .iter()
            .find_map(|value| {
                let object = value.object()?;
                (object.get("context_value_id")?.as_str()? == context_value_id)
                    .then(|| object.get("value_type")?.as_str())
                    .flatten()
            })
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "{path}: context value `{context_value_id}` does not resolve"
                ))
            }),
        SerializationValueProjection::Constant { value, .. }
        | SerializationValueProjection::Default { value, .. } => {
            serialization_typed_value_type(value, path)
        }
    }
}

fn validate_serialization_field_ref<'a>(
    document: &'a RuleSetDocument,
    field: &JsonValue,
    scopes: &[SerializationGroupScope<'_>],
    path: &str,
    profile: SerializationProfile,
) -> Result<&'a str> {
    validate_serialization_field_ref_in_group(
        document,
        field,
        scopes.last().map(|scope| scope.group_id),
        path,
        Some(profile),
    )
}

fn validate_serialization_field_ref_in_group<'a>(
    document: &'a RuleSetDocument,
    field: &JsonValue,
    current_group: Option<&str>,
    path: &str,
    profile: Option<SerializationProfile>,
) -> Result<&'a str> {
    let field_ref = required_object(field, "serialization field projection")?;
    let field_id = required_string(field_ref, "field_id", "serialization field projection")?;
    let definition = document
        .fields
        .iter()
        .find(|candidate| {
            candidate
                .object()
                .and_then(|object| object.get("field_id"))
                .and_then(JsonValue::as_str)
                == Some(field_id)
        })
        .ok_or_else(|| CodegenError::new(format!("{path}: field `{field_id}` does not resolve")))?;
    let definition = required_object(definition, "field definition")?;
    let value_type = required_string(definition, "value_type", "field definition")?;
    if let Some(profile) = profile {
        let behavior = required_object(
            definition.get("behavior").ok_or_else(|| {
                CodegenError::new(format!("{path}: field `{field_id}` is missing behavior"))
            })?,
            "serialization field behavior",
        )?;
        let branch = required_object(
            behavior.get(profile.label()).ok_or_else(|| {
                CodegenError::new(format!(
                    "{path}: field `{field_id}` is missing {} behavior",
                    profile.label()
                ))
            })?,
            "serialization field behavior branch",
        )?;
        if branch.get("state").and_then(JsonValue::as_str) != Some("executable") {
            return Err(CodegenError::new(format!(
                "{path}: field `{field_id}` behavior is not executable for {}",
                profile.label()
            )));
        }
    }
    let group_id = definition.get("group_id").and_then(JsonValue::as_str);
    let instance = required_object(
        field_ref.get("instance").ok_or_else(|| {
            CodegenError::new(format!("{path}: field projection is missing instance"))
        })?,
        "serialization field instance",
    )?;
    let kind = required_string(instance, "kind", "serialization field instance")?;
    match kind {
        "singleton" if group_id.is_none() => {}
        "current-group-instance"
            if group_id.is_some()
                && current_group.is_some_and(|current| Some(current) == group_id) => {}
        "stable-instance-id" if group_id.is_some() => {}
        _ => {
            return Err(CodegenError::new(format!(
                "{path}: field `{field_id}` instance selector `{kind}` is incompatible with declared group {group_id:?} and current dynamic scope"
            )));
        }
    }
    Ok(value_type)
}

fn parse_evaluation_scope(value: &JsonValue, path: &str) -> Result<EvaluationScope> {
    let scope = required_object(value, "evaluation scope")?;
    match required_string(scope, "kind", "evaluation scope")? {
        "singleton" => Ok(EvaluationScope::Singleton),
        "each-group" => Ok(EvaluationScope::EachGroup {
            group_id: required_string(scope, "group_id", "evaluation scope")?.to_owned(),
        }),
        kind => Err(CodegenError::new(format!(
            "{path}: unknown evaluation scope `{kind}`"
        ))),
    }
}

fn calculation_evaluation_scope(
    calculation: &BTreeMap<String, JsonValue>,
    path: &str,
) -> Result<EvaluationScope> {
    parse_evaluation_scope(
        calculation
            .get("scope")
            .ok_or_else(|| CodegenError::new(format!("{path}: calculation is missing scope")))?,
        &format!("{path}.scope"),
    )
}

fn parse_derived_instance_selector(
    value: &JsonValue,
    path: &str,
) -> Result<DerivedInstanceSelector> {
    let selector = required_object(value, "derived instance selector")?;
    match required_string(selector, "kind", "derived instance selector")? {
        "singleton" => Ok(DerivedInstanceSelector::Singleton),
        "current-group-instance" => Ok(DerivedInstanceSelector::CurrentGroupInstance),
        "stable-instance-id" => Ok(DerivedInstanceSelector::StableInstanceId {
            instance_id: required_string(selector, "instance_id", "derived instance selector")?
                .to_owned(),
        }),
        kind => Err(CodegenError::new(format!(
            "{path}: unknown derived instance selector `{kind}`"
        ))),
    }
}

fn evaluation_scope_group<'a>(scope: &'a EvaluationScope) -> Option<&'a str> {
    match scope {
        EvaluationScope::Singleton => None,
        EvaluationScope::EachGroup { group_id } => Some(group_id),
    }
}

fn validate_evaluation_scope(
    document: &RuleSetDocument,
    scope: &EvaluationScope,
    path: &str,
) -> Result<()> {
    let Some(group_id) = evaluation_scope_group(scope) else {
        return Ok(());
    };
    if document.field_groups.iter().any(|group| {
        group
            .object()
            .and_then(|group| group.get("group_id"))
            .and_then(JsonValue::as_str)
            == Some(group_id)
    }) {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "{path}: each-group scope references missing field group `{group_id}`"
        )))
    }
}

fn validate_derived_instance_selector(
    document: &RuleSetDocument,
    calculation: &BTreeMap<String, JsonValue>,
    instance: &DerivedInstanceSelector,
    current_group: Option<&str>,
    path: &str,
) -> Result<()> {
    let target_scope = calculation_evaluation_scope(calculation, path)?;
    validate_evaluation_scope(document, &target_scope, path)?;
    let compatible = match (&target_scope, instance) {
        (EvaluationScope::Singleton, DerivedInstanceSelector::Singleton) => true,
        (
            EvaluationScope::EachGroup { group_id },
            DerivedInstanceSelector::CurrentGroupInstance,
        ) => current_group == Some(group_id.as_str()),
        (EvaluationScope::EachGroup { .. }, DerivedInstanceSelector::StableInstanceId { .. }) => {
            true
        }
        _ => false,
    };
    if compatible {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "{path}: derived instance selector {instance:?} is incompatible with calculation scope {target_scope:?} and current group {current_group:?}"
        )))
    }
}

fn serialization_derived_output_type<'a>(
    document: &'a RuleSetDocument,
    target: SerializationArtifactTarget,
    profile: SerializationProfile,
    calculation_id: &str,
    output_id: &str,
    instance: &DerivedInstanceSelector,
    current_group: Option<&str>,
    path: &str,
) -> Result<&'a str> {
    let calculation = document
        .calculations
        .iter()
        .find(|candidate| {
            candidate
                .object()
                .and_then(|object| object.get("calculation_id"))
                .and_then(JsonValue::as_str)
                == Some(calculation_id)
        })
        .ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: derived calculation `{calculation_id}` does not resolve"
            ))
        })?;
    let calculation = required_object(calculation, "serialization calculation")?;
    validate_derived_instance_selector(document, calculation, instance, current_group, path)?;
    let phase = serialization_artifact_phase(target).ok_or_else(|| {
        CodegenError::new(format!(
            "{path}: derived outputs are unavailable for historical/import serialization artifacts"
        ))
    })?;
    let phases = required_string_array(calculation, "phases", "serialization calculation")?;
    if !phases.contains(&phase) {
        return Err(CodegenError::new(format!(
            "{path}: derived output `{calculation_id}.{output_id}` is not available in artifact phase `{phase}`"
        )));
    }
    let profiles = required_object(
        calculation
            .get("profiles")
            .ok_or_else(|| CodegenError::new(format!("{path}: calculation is missing profiles")))?,
        "serialization calculation profiles",
    )?;
    let branch = required_object(
        profiles.get(profile.label()).ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: calculation is missing {} branch",
                profile.label()
            ))
        })?,
        "serialization calculation branch",
    )?;
    if branch.get("state").and_then(JsonValue::as_str) != Some("executable") {
        return Err(CodegenError::new(format!(
            "{path}: derived output `{calculation_id}.{output_id}` is not executable for {}",
            profile.label()
        )));
    }
    let output = required_array(branch, "outputs", "serialization calculation branch")?
        .iter()
        .find(|candidate| {
            candidate
                .object()
                .and_then(|object| object.get("output_id"))
                .and_then(JsonValue::as_str)
                == Some(output_id)
        })
        .ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: derived output `{calculation_id}.{output_id}` does not resolve for {}",
                profile.label()
            ))
        })?;
    let output = required_object(output, "serialization calculation output")?;
    declared_expression_value_type(
        output.get("value").ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: derived output is missing value expression"
            ))
        })?,
        path,
    )
}

fn serialization_artifact_phase(target: SerializationArtifactTarget) -> Option<&'static str> {
    match target {
        SerializationArtifactTarget::EditableSave | SerializationArtifactTarget::FinalizedSave => {
            Some("save")
        }
        SerializationArtifactTarget::EncryptedFinalCopy => Some("final-copy"),
        SerializationArtifactTarget::SubmissionPayload => Some("submit"),
        SerializationArtifactTarget::HistoricalImportCompatibility => None,
    }
}

fn declared_expression_value_type<'a>(value: &'a JsonValue, path: &str) -> Result<&'a str> {
    let expression = required_object(value, "expression")?;
    if expression.get("kind").and_then(JsonValue::as_str) == Some("literal") {
        return serialization_typed_value_type(
            expression.get("value").ok_or_else(|| {
                CodegenError::new(format!("{path}: literal expression is missing value"))
            })?,
            path,
        );
    }
    required_string(expression, "result_type", "expression")
}

fn serialization_typed_value_type<'a>(value: &'a JsonValue, path: &str) -> Result<&'a str> {
    let value = required_object(value, "serialization typed value")?;
    required_string(value, "type", &format!("{path} typed value"))
}

fn validate_serialization_format(
    format: &crate::model::SerializationSemanticFormat,
    value_type: &str,
    path: &str,
) -> Result<()> {
    let expected = match &format.present {
        SerializationPresentFormat::Text => "string",
        SerializationPresentFormat::Boolean {
            true_text,
            false_text,
        } => {
            if true_text == false_text {
                return Err(CodegenError::new(format!(
                    "{path}: boolean true_text and false_text must be distinguishable"
                )));
            }
            "boolean"
        }
        SerializationPresentFormat::Base10Integer => "integer",
        SerializationPresentFormat::Decimal {
            grouping,
            decimal_separator,
            ..
        } => {
            if matches!(
                (grouping, decimal_separator),
                (
                    SerializationGrouping::Comma,
                    SerializationDecimalSeparator::Comma
                ) | (
                    SerializationGrouping::Period,
                    SerializationDecimalSeparator::Period
                )
            ) {
                return Err(CodegenError::new(format!(
                    "{path}: decimal grouping separator must differ from decimal_separator"
                )));
            }
            "decimal"
        }
        SerializationPresentFormat::Date { .. } => "date",
    };
    if value_type != expected {
        return Err(CodegenError::new(format!(
            "{path}: serialization formatter/value type mismatch: formatter expects `{expected}`, projection provides `{value_type}`"
        )));
    }
    Ok(())
}

fn validate_serialization_presence(
    document: &RuleSetDocument,
    target: SerializationArtifactTarget,
    profile: SerializationProfile,
    presence: &SerializationPresence,
    scopes: &[SerializationGroupScope<'_>],
    path: &str,
) -> Result<()> {
    let SerializationPresence::When { predicate } = presence else {
        return Ok(());
    };
    validate_serialization_predicate(
        document,
        target,
        profile,
        predicate,
        scopes.last().map(|scope| scope.group_id),
        &format!("{path}.predicate"),
    )
}

fn validate_serialization_predicate(
    document: &RuleSetDocument,
    target: SerializationArtifactTarget,
    profile: SerializationProfile,
    predicate: &JsonValue,
    current_group: Option<&str>,
    path: &str,
) -> Result<()> {
    let object = required_object(predicate, "serialization presence predicate")?;
    let kind = required_string(object, "kind", "serialization presence predicate")?;
    match kind {
        "constant" => Ok(()),
        "not" => validate_serialization_predicate(
            document,
            target,
            profile,
            object.get("predicate").ok_or_else(|| {
                CodegenError::new(format!("{path}: not predicate is missing predicate"))
            })?,
            current_group,
            &format!("{path}.predicate"),
        ),
        "all" | "any" => {
            for (index, child) in
                required_array(object, "predicates", "serialization logical predicate")?
                    .iter()
                    .enumerate()
            {
                validate_serialization_predicate(
                    document,
                    target,
                    profile,
                    child,
                    current_group,
                    &format!("{path}.predicates[{index}]"),
                )?;
            }
            Ok(())
        }
        "compare" => {
            let operator = required_string(object, "operator", "serialization compare predicate")?;
            let left = validate_serialization_expression(
                document,
                target,
                profile,
                object
                    .get("left")
                    .ok_or_else(|| CodegenError::new(format!("{path}: missing compare left")))?,
                current_group,
                &format!("{path}.left"),
            )?;
            let right = validate_serialization_expression(
                document,
                target,
                profile,
                object
                    .get("right")
                    .ok_or_else(|| CodegenError::new(format!("{path}: missing compare right")))?,
                current_group,
                &format!("{path}.right"),
            )?;
            if left != right {
                return Err(CodegenError::new(format!(
                    "{path}: compare predicate type mismatch `{left}` versus `{right}`"
                )));
            }
            validate_serialization_compare_operator(operator, &left, path)?;
            Ok(())
        }
        "is-empty" | "is-present" | "is-null" => {
            validate_serialization_expression(
                document,
                target,
                profile,
                object.get("value").ok_or_else(|| {
                    CodegenError::new(format!("{path}: presence predicate is missing value"))
                })?,
                current_group,
                &format!("{path}.value"),
            )?;
            Ok(())
        }
        "coercion-failed" => {
            let field = object.get("field").ok_or_else(|| {
                CodegenError::new(format!(
                    "{path}: coercion-failed predicate is missing field"
                ))
            })?;
            validate_serialization_field_ref_in_group(
                document,
                field,
                current_group,
                &format!("{path}.field"),
                Some(profile),
            )?;
            ensure_serialization_coercion_failure_observable(
                document,
                field,
                profile,
                &format!("{path}.field"),
            )
        }
        "matches" => Err(CodegenError::new(format!(
            "{path}: unsupported executable serialization presence predicate `matches`; no audited packaged regex backend exists"
        ))),
        "in" => {
            let value_type = validate_serialization_expression(
                document,
                target,
                profile,
                object.get("value").ok_or_else(|| {
                    CodegenError::new(format!("{path}: membership predicate is missing value"))
                })?,
                current_group,
                &format!("{path}.value"),
            )?;
            for (index, candidate) in
                required_array(object, "candidates", "serialization membership predicate")?
                    .iter()
                    .enumerate()
            {
                let candidate_type = serialization_typed_value_type(
                    candidate,
                    &format!("{path}.candidates[{index}]"),
                )?;
                if candidate_type != value_type {
                    return Err(CodegenError::new(format!(
                        "{path}.candidates[{index}]: membership candidate type `{candidate_type}` differs from expression type `{value_type}`"
                    )));
                }
            }
            Ok(())
        }
        "group-quantifier" => {
            let group_id = required_string(object, "group_id", "serialization group quantifier")?;
            serialization_group_bounds(document, group_id, path)?;
            validate_serialization_predicate(
                document,
                target,
                profile,
                object.get("predicate").ok_or_else(|| {
                    CodegenError::new(format!("{path}: group quantifier is missing predicate"))
                })?,
                Some(group_id),
                &format!("{path}.predicate"),
            )
        }
        other => Err(CodegenError::new(format!(
            "{path}: unknown executable serialization predicate `{other}`"
        ))),
    }
}

fn validate_serialization_compare_operator(
    operator: &str,
    operand_type: &str,
    path: &str,
) -> Result<()> {
    let valid = match operator {
        "equal" | "not-equal" => true,
        "less-than" | "less-than-or-equal" | "greater-than" | "greater-than-or-equal" => {
            matches!(operand_type, "string" | "integer" | "decimal" | "date")
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "{path}: compare operator `{operator}` is invalid for `{operand_type}`"
        )))
    }
}

fn validate_serialization_expression(
    document: &RuleSetDocument,
    target: SerializationArtifactTarget,
    profile: SerializationProfile,
    expression: &JsonValue,
    current_group: Option<&str>,
    path: &str,
) -> Result<String> {
    let object = required_object(expression, "serialization predicate expression")?;
    let kind = required_string(object, "kind", "serialization predicate expression")?;
    match kind {
        "literal" => serialization_typed_value_type(
            object
                .get("value")
                .ok_or_else(|| CodegenError::new(format!("{path}: literal is missing value")))?,
            path,
        )
        .map(str::to_owned),
        "field" => {
            let actual = validate_serialization_field_ref_in_group(
                document,
                object
                    .get("field")
                    .ok_or_else(|| CodegenError::new(format!("{path}: field is missing ref")))?,
                current_group,
                &format!("{path}.field"),
                Some(profile),
            )?;
            ensure_declared_serialization_expression_type(object, actual, path)
        }
        "derived" => {
            let calculation_id =
                required_string(object, "calculation_id", "serialization derived expression")?;
            let output_id =
                required_string(object, "output_id", "serialization derived expression")?;
            let instance = parse_derived_instance_selector(
                object.get("instance").ok_or_else(|| {
                    CodegenError::new(format!(
                        "{path}: serialization derived expression is missing instance"
                    ))
                })?,
                &format!("{path}.instance"),
            )?;
            let actual = serialization_derived_output_type(
                document,
                target,
                profile,
                calculation_id,
                output_id,
                &instance,
                current_group,
                path,
            )?;
            ensure_declared_serialization_expression_type(object, actual, path)
        }
        "context" => {
            let context_id = required_string(
                object,
                "context_value_id",
                "serialization context expression",
            )?;
            let actual = serialization_context_value_type(document, context_id, path)?;
            ensure_declared_serialization_expression_type(object, actual, path)
        }
        "unary" => {
            let operand = validate_serialization_expression(
                document,
                target,
                profile,
                object
                    .get("operand")
                    .ok_or_else(|| CodegenError::new(format!("{path}: unary missing operand")))?,
                current_group,
                &format!("{path}.operand"),
            )?;
            let operator = required_string(object, "operator", "serialization unary expression")?;
            let expected = match operator {
                "length" if operand == "string" => "integer",
                "negate" | "absolute" if matches!(operand.as_str(), "integer" | "decimal") => {
                    operand.as_str()
                }
                _ => {
                    return Err(CodegenError::new(format!(
                        "{path}: unary operator `{operator}` is invalid for `{operand}`"
                    )));
                }
            };
            ensure_declared_serialization_expression_type(object, expected, path)
        }
        "binary" => {
            let left = validate_serialization_expression(
                document,
                target,
                profile,
                object
                    .get("left")
                    .ok_or_else(|| CodegenError::new(format!("{path}: binary missing left")))?,
                current_group,
                &format!("{path}.left"),
            )?;
            let right = validate_serialization_expression(
                document,
                target,
                profile,
                object
                    .get("right")
                    .ok_or_else(|| CodegenError::new(format!("{path}: binary missing right")))?,
                current_group,
                &format!("{path}.right"),
            )?;
            let operator = required_string(object, "operator", "serialization binary expression")?;
            let expected = if operator == "concat" && left == "string" && right == "string" {
                "string"
            } else if matches!(
                operator,
                "add" | "subtract" | "multiply" | "divide" | "remainder"
            ) && left == right
                && matches!(left.as_str(), "integer" | "decimal")
            {
                left.as_str()
            } else {
                return Err(CodegenError::new(format!(
                    "{path}: binary operator `{operator}` has incompatible operand types `{left}` and `{right}`"
                )));
            };
            ensure_declared_serialization_expression_type(object, expected, path)
        }
        "nary" => {
            let operator = required_string(object, "operator", "serialization nary expression")?;
            let mut operand_type: Option<String> = None;
            for (index, operand) in
                required_array(object, "operands", "serialization nary expression")?
                    .iter()
                    .enumerate()
            {
                let current = validate_serialization_expression(
                    document,
                    target,
                    profile,
                    operand,
                    current_group,
                    &format!("{path}.operands[{index}]"),
                )?;
                if operand_type
                    .as_ref()
                    .is_some_and(|expected| expected != &current)
                {
                    return Err(CodegenError::new(format!(
                        "{path}: nary operands have mismatched types"
                    )));
                }
                operand_type = Some(current);
            }
            let operand_type = operand_type.ok_or_else(|| {
                CodegenError::new(format!("{path}: nary expression has no operands"))
            })?;
            let valid = match operator {
                "concat" => operand_type == "string",
                "sum" => matches!(operand_type.as_str(), "integer" | "decimal"),
                "minimum" | "maximum" | "coalesce" => operand_type != "null",
                _ => false,
            };
            if !valid {
                return Err(CodegenError::new(format!(
                    "{path}: nary operator `{operator}` is invalid for `{operand_type}`"
                )));
            }
            ensure_declared_serialization_expression_type(object, &operand_type, path)
        }
        "conditional" => {
            validate_serialization_predicate(
                document,
                target,
                profile,
                object.get("condition").ok_or_else(|| {
                    CodegenError::new(format!("{path}: conditional missing condition"))
                })?,
                current_group,
                &format!("{path}.condition"),
            )?;
            let when_true = validate_serialization_expression(
                document,
                target,
                profile,
                object.get("when_true").ok_or_else(|| {
                    CodegenError::new(format!("{path}: conditional missing when_true"))
                })?,
                current_group,
                &format!("{path}.when_true"),
            )?;
            let when_false = validate_serialization_expression(
                document,
                target,
                profile,
                object.get("when_false").ok_or_else(|| {
                    CodegenError::new(format!("{path}: conditional missing when_false"))
                })?,
                current_group,
                &format!("{path}.when_false"),
            )?;
            if when_true != when_false {
                return Err(CodegenError::new(format!(
                    "{path}: conditional branch type mismatch `{when_true}` versus `{when_false}`"
                )));
            }
            ensure_declared_serialization_expression_type(object, &when_true, path)
        }
        "coerce" => {
            validate_serialization_expression(
                document,
                target,
                profile,
                object
                    .get("input")
                    .ok_or_else(|| CodegenError::new(format!("{path}: coerce missing input")))?,
                current_group,
                &format!("{path}.input"),
            )?;
            let coercion = required_object(
                object
                    .get("coercion")
                    .ok_or_else(|| CodegenError::new(format!("{path}: coerce missing policy")))?,
                "serialization coercion",
            )?;
            let expected = required_string(coercion, "kind", "serialization coercion")?;
            ensure_declared_serialization_expression_type(object, expected, path)
        }
        "group-aggregate" => {
            let group_id = required_string(object, "group_id", "serialization group aggregate")?;
            serialization_group_bounds(document, group_id, path)?;
            if current_group.is_some() {
                return Err(CodegenError::new(format!(
                    "{path}: nested group aggregate is not supported"
                )));
            }
            let value_type = validate_serialization_expression(
                document,
                target,
                profile,
                object.get("value").ok_or_else(|| {
                    CodegenError::new(format!("{path}: group aggregate is missing value"))
                })?,
                Some(group_id),
                &format!("{path}.value"),
            )?;
            let operator = required_string(object, "operator", "serialization group aggregate")?;
            let expected = match operator {
                "count" | "count-present" => "integer",
                "sum" if matches!(value_type.as_str(), "integer" | "decimal") => {
                    value_type.as_str()
                }
                "minimum" | "maximum"
                    if matches!(
                        value_type.as_str(),
                        "string" | "integer" | "decimal" | "date"
                    ) =>
                {
                    value_type.as_str()
                }
                _ => {
                    return Err(CodegenError::new(format!(
                        "{path}: aggregate operator `{operator}` is invalid for `{value_type}`"
                    )));
                }
            };
            ensure_declared_serialization_expression_type(object, expected, path)
        }
        other => Err(CodegenError::new(format!(
            "{path}: unknown executable serialization expression `{other}`"
        ))),
    }
}

fn ensure_declared_serialization_expression_type(
    expression: &BTreeMap<String, JsonValue>,
    actual: &str,
    path: &str,
) -> Result<String> {
    let declared = required_string(
        expression,
        "result_type",
        "serialization predicate expression",
    )?;
    if declared != actual {
        return Err(CodegenError::new(format!(
            "{path}: expression result_type `{declared}` does not match referenced type `{actual}`"
        )));
    }
    Ok(actual.to_owned())
}

fn serialization_context_value_type<'a>(
    document: &'a RuleSetDocument,
    context_value_id: &str,
    path: &str,
) -> Result<&'a str> {
    document
        .context_values
        .iter()
        .find_map(|value| {
            let object = value.object()?;
            (object.get("context_value_id")?.as_str()? == context_value_id)
                .then(|| object.get("value_type")?.as_str())
                .flatten()
        })
        .ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: context value `{context_value_id}` does not resolve"
            ))
        })
}

fn serialization_field_definition<'a>(
    document: &'a RuleSetDocument,
    field_id: &str,
    path: &str,
) -> Result<&'a BTreeMap<String, JsonValue>> {
    document
        .fields
        .iter()
        .find_map(|candidate| {
            let object = candidate.object()?;
            (object.get("field_id")?.as_str()? == field_id).then_some(object)
        })
        .ok_or_else(|| CodegenError::new(format!("{path}: field `{field_id}` does not resolve")))
}

fn ensure_serialization_coercion_failure_observable(
    document: &RuleSetDocument,
    field: &JsonValue,
    profile: SerializationProfile,
    path: &str,
) -> Result<()> {
    let field_ref = required_object(field, "serialization coercion-failed field")?;
    let field_id = required_string(field_ref, "field_id", "serialization coercion-failed field")?;
    let definition = serialization_field_definition(document, field_id, path)?;
    let behavior = required_object(
        definition
            .get("behavior")
            .ok_or_else(|| CodegenError::new(format!("{path}: field behavior is missing")))?,
        "serialization field behavior",
    )?;
    let branch = required_object(
        behavior.get(profile.label()).ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: field is missing {} behavior",
                profile.label()
            ))
        })?,
        "serialization field behavior branch",
    )?;
    let coercion = required_object(
        branch
            .get("coercion")
            .ok_or_else(|| CodegenError::new(format!("{path}: field coercion is missing")))?,
        "serialization field coercion",
    )?;
    if coercion.get("on_invalid").and_then(JsonValue::as_str) != Some("preserve-raw") {
        return Err(CodegenError::new(format!(
            "{path}: coercion-failed requires on_invalid `preserve-raw` for {}",
            profile.label()
        )));
    }
    Ok(())
}

fn serialization_group_bounds(
    document: &RuleSetDocument,
    group_id: &str,
    path: &str,
) -> Result<(usize, Option<usize>)> {
    let group = document
        .field_groups
        .iter()
        .find(|candidate| {
            candidate
                .object()
                .and_then(|object| object.get("group_id"))
                .and_then(JsonValue::as_str)
                == Some(group_id)
        })
        .ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: dynamic group `{group_id}` does not resolve"
            ))
        })?;
    let group = required_object(group, "serialization field group")?;
    let minimum = usize::try_from(required_u64(
        group,
        "min_occurs",
        "serialization field group",
    )?)
    .map_err(|_| CodegenError::new(format!("{path}: min_occurs exceeds usize")))?;
    let maximum = match group.get("max_occurs") {
        Some(JsonValue::Null) => None,
        Some(JsonValue::Number(number)) => Some(
            number
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| CodegenError::new(format!("{path}: max_occurs exceeds usize")))?,
        ),
        _ => {
            return Err(CodegenError::new(format!(
                "{path}: field group max_occurs is invalid after schema validation"
            )));
        }
    };
    Ok((minimum, maximum))
}

fn validate_scoped_evaluation_contract(document: &RuleSetDocument) -> Result<()> {
    for (index, calculation) in document.calculations.iter().enumerate() {
        let path = format!("$.calculations[{index}]");
        let calculation = required_object(calculation, "calculation")?;
        let calculation_id = required_string(calculation, "calculation_id", "calculation")?;
        let scope = calculation_evaluation_scope(calculation, &path)?;
        validate_evaluation_scope(document, &scope, &format!("{path}.scope"))?;
        let phases = required_string_array(calculation, "phases", "calculation")?;
        validate_scoped_profile_branches(
            document,
            calculation,
            evaluation_scope_group(&scope),
            Some(calculation_id),
            &phases,
            &path,
        )?;
    }

    for (index, rule) in document.rules.iter().enumerate() {
        let path = format!("$.rules[{index}]");
        let rule = required_object(rule, "rule")?;
        let scope = parse_evaluation_scope(
            rule.get("scope")
                .ok_or_else(|| CodegenError::new(format!("{path}: rule is missing scope")))?,
            &format!("{path}.scope"),
        )?;
        validate_evaluation_scope(document, &scope, &format!("{path}.scope"))?;
        let phases = required_string_array(rule, "phases", "rule")?;
        validate_scoped_profile_branches(
            document,
            rule,
            evaluation_scope_group(&scope),
            None,
            &phases,
            &path,
        )?;
    }
    Ok(())
}

fn validate_scoped_profile_branches(
    document: &RuleSetDocument,
    definition: &BTreeMap<String, JsonValue>,
    current_group: Option<&str>,
    owner_calculation_id: Option<&str>,
    phases: &[&str],
    path: &str,
) -> Result<()> {
    let profiles = required_object(
        definition
            .get("profiles")
            .ok_or_else(|| CodegenError::new(format!("{path}: missing profiles")))?,
        "scoped executable profiles",
    )?;
    for profile in ["official", "filing_safe"] {
        let branch = required_object(
            profiles
                .get(profile)
                .ok_or_else(|| CodegenError::new(format!("{path}: missing {profile} profile")))?,
            "scoped executable profile",
        )?;
        if branch.get("state").and_then(JsonValue::as_str) != Some("executable") {
            continue;
        }
        validate_scoped_node(
            document,
            profiles
                .get(profile)
                .expect("profile branch was resolved above"),
            current_group,
            owner_calculation_id,
            profile,
            phases,
            &format!("{path}.profiles.{profile}"),
        )?;
    }
    Ok(())
}

fn validate_scoped_node(
    document: &RuleSetDocument,
    value: &JsonValue,
    current_group: Option<&str>,
    owner_calculation_id: Option<&str>,
    profile: &str,
    phases: &[&str],
    path: &str,
) -> Result<()> {
    match value {
        JsonValue::Object(object) => {
            match object.get("kind").and_then(JsonValue::as_str) {
                Some("group-aggregate") => {
                    if current_group.is_some() {
                        return Err(CodegenError::new(format!(
                            "{path}: nested group aggregate is not supported"
                        )));
                    }
                    let group_id =
                        required_string(object, "group_id", "group aggregate expression")?;
                    ensure_field_group_exists(document, group_id, path)?;
                    let nested = object.get("value").ok_or_else(|| {
                        CodegenError::new(format!("{path}: group aggregate is missing value"))
                    })?;
                    let value_type =
                        declared_expression_value_type(nested, &format!("{path}.value"))?;
                    let result_type =
                        required_string(object, "result_type", "group aggregate expression")?;
                    let operator =
                        required_string(object, "operator", "group aggregate expression")?;
                    let expected_type = match operator {
                        "count" | "count-present" => "integer",
                        "sum" if matches!(value_type, "integer" | "decimal") => value_type,
                        "minimum" | "maximum"
                            if matches!(value_type, "string" | "integer" | "decimal" | "date") =>
                        {
                            value_type
                        }
                        _ => {
                            return Err(CodegenError::new(format!(
                                "{path}: group aggregate operator `{operator}` is invalid for value type `{value_type}`"
                            )));
                        }
                    };
                    if result_type != expected_type {
                        return Err(CodegenError::new(format!(
                            "{path}: group aggregate result_type `{result_type}` must be `{expected_type}` for operator `{operator}`"
                        )));
                    }
                    return validate_scoped_node(
                        document,
                        nested,
                        Some(group_id),
                        owner_calculation_id,
                        profile,
                        phases,
                        &format!("{path}.value"),
                    );
                }
                Some("group-quantifier") => {
                    let group_id =
                        required_string(object, "group_id", "group quantifier predicate")?;
                    ensure_field_group_exists(document, group_id, path)?;
                    let nested = object.get("predicate").ok_or_else(|| {
                        CodegenError::new(format!("{path}: group quantifier is missing predicate"))
                    })?;
                    return validate_scoped_node(
                        document,
                        nested,
                        Some(group_id),
                        owner_calculation_id,
                        profile,
                        phases,
                        &format!("{path}.predicate"),
                    );
                }
                Some("split-component") => {
                    let result_type =
                        required_string(object, "result_type", "split component expression")?;
                    let delimiter =
                        required_string(object, "delimiter", "split component expression")?;
                    let input = object.get("input").ok_or_else(|| {
                        CodegenError::new(format!("{path}: split component is missing input"))
                    })?;
                    let input_type =
                        declared_expression_value_type(input, &format!("{path}.input"))?;
                    if result_type != "string" || delimiter != "/" {
                        return Err(CodegenError::new(format!(
                            "{path}: split component requires string result_type and literal `/` delimiter"
                        )));
                    }
                    if !matches!(input_type, "string" | "null") {
                        return Err(CodegenError::new(format!(
                            "{path}: split component input type `{input_type}` must be string"
                        )));
                    }
                }
                Some("javascript-parse-float") => {
                    let input = object.get("input").ok_or_else(|| {
                        CodegenError::new(format!(
                            "{path}: JavaScript parseFloat predicate is missing input"
                        ))
                    })?;
                    let input_type =
                        declared_expression_value_type(input, &format!("{path}.input"))?;
                    if !matches!(input_type, "string" | "null") {
                        return Err(CodegenError::new(format!(
                            "{path}: JavaScript parseFloat requires string input"
                        )));
                    }
                    let operator =
                        required_string(object, "operator", "JavaScript parseFloat predicate")?;
                    match operator {
                        "is-nan" if !object.contains_key("operand") => {}
                        "strict-equal" | "greater-than" => {
                            let operand = object.get("operand").ok_or_else(|| {
                                CodegenError::new(format!(
                                    "{path}: JavaScript parseFloat operator `{operator}` is missing operand"
                                ))
                            })?;
                            if serialization_typed_value_type(operand, &format!("{path}.operand"))?
                                != "decimal"
                            {
                                return Err(CodegenError::new(format!(
                                    "{path}: JavaScript parseFloat operand must be decimal"
                                )));
                            }
                        }
                        _ => {
                            return Err(CodegenError::new(format!(
                                "{path}: invalid JavaScript parseFloat operator/operand shape"
                            )));
                        }
                    }
                }
                Some("javascript-parse-int-radix10") => {
                    let result_type =
                        required_string(object, "result_type", "JavaScript parseInt expression")?;
                    let input = object.get("input").ok_or_else(|| {
                        CodegenError::new(format!("{path}: JavaScript parseInt is missing input"))
                    })?;
                    let input_type =
                        declared_expression_value_type(input, &format!("{path}.input"))?;
                    if result_type != "integer" || !matches!(input_type, "string" | "null") {
                        return Err(CodegenError::new(format!(
                            "{path}: JavaScript parseInt requires string input and integer result_type"
                        )));
                    }
                }
                Some("javascript-date-local-day") => {
                    let result_type =
                        required_string(object, "result_type", "JavaScript local date expression")?;
                    if result_type != "integer" {
                        return Err(CodegenError::new(format!(
                            "{path}: JavaScript local date requires integer result_type"
                        )));
                    }
                    for component in ["year", "month_index", "day"] {
                        let nested = object.get(component).ok_or_else(|| {
                            CodegenError::new(format!(
                                "{path}: JavaScript local date is missing {component}"
                            ))
                        })?;
                        let nested_type =
                            declared_expression_value_type(nested, &format!("{path}.{component}"))?;
                        if !matches!(nested_type, "integer" | "null") {
                            return Err(CodegenError::new(format!(
                                "{path}.{component}: JavaScript local date component type `{nested_type}` must be integer"
                            )));
                        }
                    }
                }
                Some("canonical-local-date-day") => {
                    let result_type =
                        required_string(object, "result_type", "canonical local date expression")?;
                    let input = object.get("input").ok_or_else(|| {
                        CodegenError::new(format!("{path}: canonical local date is missing input"))
                    })?;
                    let input_type =
                        declared_expression_value_type(input, &format!("{path}.input"))?;
                    if result_type != "integer" || !matches!(input_type, "date" | "null") {
                        return Err(CodegenError::new(format!(
                            "{path}: canonical local date requires date input and integer result_type"
                        )));
                    }
                }
                Some("derived") => validate_scoped_derived_reference(
                    document,
                    object,
                    current_group,
                    owner_calculation_id,
                    profile,
                    phases,
                    path,
                )?,
                _ => {
                    if object.contains_key("field_id") && object.contains_key("instance") {
                        validate_scoped_field_ref(document, object, current_group, path)?;
                    }
                }
            }
            for (key, child) in object {
                validate_scoped_node(
                    document,
                    child,
                    current_group,
                    owner_calculation_id,
                    profile,
                    phases,
                    &format!("{path}.{key}"),
                )?;
            }
            Ok(())
        }
        JsonValue::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                validate_scoped_node(
                    document,
                    child,
                    current_group,
                    owner_calculation_id,
                    profile,
                    phases,
                    &format!("{path}[{index}]"),
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn ensure_field_group_exists(document: &RuleSetDocument, group_id: &str, path: &str) -> Result<()> {
    if document.field_groups.iter().any(|group| {
        group
            .object()
            .and_then(|group| group.get("group_id"))
            .and_then(JsonValue::as_str)
            == Some(group_id)
    }) {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "{path}: field group `{group_id}` does not resolve"
        )))
    }
}

fn validate_scoped_field_ref(
    document: &RuleSetDocument,
    field_ref: &BTreeMap<String, JsonValue>,
    current_group: Option<&str>,
    path: &str,
) -> Result<()> {
    let field_id = required_string(field_ref, "field_id", "field reference")?;
    let field = serialization_field_definition(document, field_id, path)?;
    let field_group = field.get("group_id").and_then(JsonValue::as_str);
    let selector = required_object(
        field_ref
            .get("instance")
            .ok_or_else(|| CodegenError::new(format!("{path}: field ref is missing instance")))?,
        "field instance selector",
    )?;
    let selector_kind = required_string(selector, "kind", "field instance selector")?;
    let compatible = match (field_group, selector_kind) {
        (None, "singleton") => true,
        (Some(group_id), "current-group-instance") => current_group == Some(group_id),
        (Some(_), "stable-instance-id") => true,
        _ => false,
    };
    if compatible {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "{path}: field `{field_id}` instance selector `{selector_kind}` is incompatible with declared group {field_group:?} and current group {current_group:?}"
        )))
    }
}

fn validate_scoped_derived_reference(
    document: &RuleSetDocument,
    derived: &BTreeMap<String, JsonValue>,
    current_group: Option<&str>,
    owner_calculation_id: Option<&str>,
    profile: &str,
    phases: &[&str],
    path: &str,
) -> Result<()> {
    let calculation_id = required_string(derived, "calculation_id", "derived expression")?;
    let output_id = required_string(derived, "output_id", "derived expression")?;
    let calculation = document
        .calculations
        .iter()
        .find_map(|candidate| {
            let object = candidate.object()?;
            (object.get("calculation_id")?.as_str()? == calculation_id).then_some(object)
        })
        .ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: derived calculation `{calculation_id}` does not resolve"
            ))
        })?;
    let instance = parse_derived_instance_selector(
        derived.get("instance").ok_or_else(|| {
            CodegenError::new(format!("{path}: derived expression is missing instance"))
        })?,
        &format!("{path}.instance"),
    )?;
    validate_derived_instance_selector(document, calculation, &instance, current_group, path)?;

    let declared_outputs = required_string_array(calculation, "output_ids", "calculation")?;
    if !declared_outputs.contains(&output_id) {
        return Err(CodegenError::new(format!(
            "{path}: derived output `{calculation_id}.{output_id}` does not resolve"
        )));
    }
    let target_phases = required_string_array(calculation, "phases", "calculation")?;
    if let Some(phase) = phases.iter().find(|phase| !target_phases.contains(phase)) {
        return Err(CodegenError::new(format!(
            "{path}: derived calculation `{calculation_id}` is unavailable in owner phase `{phase}`"
        )));
    }
    let branch = profile_branch(calculation, profile, "derived calculation")?;
    if branch.get("state").and_then(JsonValue::as_str) != Some("executable") {
        return Err(CodegenError::new(format!(
            "{path}: derived calculation `{calculation_id}` is not executable for profile `{profile}`"
        )));
    }
    let branch_outputs = required_array(branch, "outputs", "derived calculation profile")?;
    let output = branch_outputs
        .iter()
        .find(|output| {
            output
                .object()
                .and_then(|output| output.get("output_id"))
                .and_then(JsonValue::as_str)
                == Some(output_id)
        })
        .ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: derived output `{calculation_id}.{output_id}` is not executable for profile `{profile}`"
            ))
        })?;
    let output = required_object(output, "derived calculation output")?;
    let output_type = declared_expression_value_type(
        output.get("value").ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: derived output `{calculation_id}.{output_id}` is missing value"
            ))
        })?,
        path,
    )?;
    let declared_type = required_string(derived, "result_type", "derived expression")?;
    if declared_type != output_type {
        return Err(CodegenError::new(format!(
            "{path}: derived expression result_type `{declared_type}` differs from `{calculation_id}.{output_id}` type `{output_type}`"
        )));
    }
    if let Some(owner_id) = owner_calculation_id {
        let owner = document
            .calculations
            .iter()
            .find_map(|candidate| {
                let object = candidate.object()?;
                (object.get("calculation_id")?.as_str()? == owner_id).then_some(object)
            })
            .ok_or_else(|| CodegenError::new("calculation owner does not resolve"))?;
        if !required_string_array(owner, "depends_on", "calculation")?.contains(&calculation_id) {
            return Err(CodegenError::new(format!(
                "{path}: calculation `{owner_id}` does not declare derived dependency `{calculation_id}`"
            )));
        }
    }
    Ok(())
}

fn validate_calculation_graph(document: &RuleSetDocument) -> Result<()> {
    let ids = collect_object_ids(&document.calculations, "calculation_id")?;
    let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    for calculation in &document.calculations {
        let object = required_object(calculation, "calculation")?;
        let id = required_string(object, "calculation_id", "calculation")?.to_owned();
        let declared = required_string_array(object, "depends_on", "calculation")?
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        for dependency in &declared {
            if !ids.contains(dependency.as_str()) {
                return Err(CodegenError::new(format!(
                    "calculation `{id}` depends on missing calculation `{dependency}`"
                )));
            }
            validate_declared_dependency_availability(document, object, &id, dependency)?;
        }
        let mut expression_dependencies = BTreeSet::new();
        collect_derived_calculation_ids(calculation, &mut expression_dependencies);
        if declared != expression_dependencies {
            return Err(CodegenError::new(format!(
                "calculation `{id}` depends_on {:?} does not exactly match derived-expression dependencies {:?}",
                declared, expression_dependencies
            )));
        }
        dependencies.insert(id, declared);
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut stack = Vec::new();
    for id in dependencies.keys() {
        visit_calculation(id, &dependencies, &mut visiting, &mut visited, &mut stack)?;
    }

    let expected_order = dependencies.keys().cloned().collect::<BTreeSet<_>>();
    let actual_order = document
        .evaluation_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if expected_order != actual_order || actual_order.len() != document.evaluation_order.len() {
        return Err(CodegenError::new(
            "evaluation_order must contain every calculation exactly once",
        ));
    }
    let positions = document
        .evaluation_order
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    for (id, dependencies) in &dependencies {
        for dependency in dependencies {
            if positions[dependency.as_str()] >= positions[id.as_str()] {
                return Err(CodegenError::new(format!(
                    "evaluation_order places `{id}` before dependency `{dependency}`"
                )));
            }
        }
    }
    Ok(())
}

fn validate_declared_dependency_availability(
    document: &RuleSetDocument,
    owner: &BTreeMap<String, JsonValue>,
    owner_id: &str,
    dependency_id: &str,
) -> Result<()> {
    let dependency = document
        .calculations
        .iter()
        .find_map(|candidate| {
            let object = candidate.object()?;
            (object.get("calculation_id")?.as_str()? == dependency_id).then_some(object)
        })
        .ok_or_else(|| {
            CodegenError::new(format!(
                "calculation `{owner_id}` depends on missing calculation `{dependency_id}`"
            ))
        })?;
    let owner_phases = required_string_array(owner, "phases", "calculation")?;
    let dependency_phases = required_string_array(dependency, "phases", "calculation")?;
    let owner_profiles = required_object(
        owner
            .get("profiles")
            .ok_or_else(|| CodegenError::new("calculation is missing profiles"))?,
        "calculation profiles",
    )?;
    let dependency_profiles = required_object(
        dependency
            .get("profiles")
            .ok_or_else(|| CodegenError::new("dependency calculation is missing profiles"))?,
        "dependency calculation profiles",
    )?;
    for profile in ["official", "filing_safe"] {
        let owner_branch = required_object(
            owner_profiles
                .get(profile)
                .ok_or_else(|| CodegenError::new("calculation profile is missing"))?,
            "calculation profile",
        )?;
        if owner_branch.get("state").and_then(JsonValue::as_str) != Some("executable") {
            continue;
        }
        let dependency_branch = required_object(
            dependency_profiles
                .get(profile)
                .ok_or_else(|| CodegenError::new("dependency calculation profile is missing"))?,
            "dependency calculation profile",
        )?;
        if dependency_branch.get("state").and_then(JsonValue::as_str) != Some("executable") {
            return Err(CodegenError::new(format!(
                "calculation `{owner_id}` executable `{profile}` dependency `{dependency_id}` is not executable"
            )));
        }
        if let Some(phase) = owner_phases
            .iter()
            .find(|phase| !dependency_phases.contains(phase))
        {
            return Err(CodegenError::new(format!(
                "calculation `{owner_id}` dependency `{dependency_id}` is unavailable in phase `{phase}`"
            )));
        }
    }
    Ok(())
}

fn visit_calculation(
    id: &str,
    dependencies: &BTreeMap<String, BTreeSet<String>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
) -> Result<()> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.to_owned()) {
        let start = stack
            .iter()
            .position(|candidate| candidate == id)
            .unwrap_or(0);
        let mut cycle = stack[start..].to_vec();
        cycle.push(id.to_owned());
        return Err(CodegenError::new(format!(
            "calculation dependency cycle: {}",
            cycle.join(" -> ")
        )));
    }
    stack.push(id.to_owned());
    for dependency in &dependencies[id] {
        visit_calculation(dependency, dependencies, visiting, visited, stack)?;
    }
    stack.pop();
    visiting.remove(id);
    visited.insert(id.to_owned());
    Ok(())
}

fn validate_field_coverage(document: &RuleSetDocument) -> Result<()> {
    validated_unbounded_family_member_count(document)?;
    validate_serialized_occurrences(document)
}

fn validated_unbounded_family_member_count(document: &RuleSetDocument) -> Result<usize> {
    let mut fields = BTreeMap::new();
    for field in &document.fields {
        let object = required_object(field, "field")?;
        let id = required_string(object, "field_id", "field")?.to_owned();
        let group = match object.get("group_id") {
            Some(JsonValue::String(group)) => Some(group.clone()),
            Some(JsonValue::Null) => None,
            _ => {
                return Err(CodegenError::new(format!(
                    "field `{id}` has invalid group_id"
                )));
            }
        };
        if fields.insert(id.clone(), group).is_some() {
            return Err(CodegenError::new(format!("duplicate field_id `{id}`")));
        }
    }

    let mut memberships = BTreeMap::<String, String>::new();
    let mut group_ids = BTreeSet::new();
    let mut unbounded_members = BTreeSet::new();
    for group in &document.field_groups {
        let object = required_object(group, "field group")?;
        let group_id = required_string(object, "group_id", "field group")?;
        if !group_ids.insert(group_id) {
            return Err(CodegenError::new(format!(
                "duplicate field group_id `{group_id}`"
            )));
        }
        let is_unbounded = match object.get("max_occurs") {
            Some(JsonValue::Null) => true,
            Some(JsonValue::Number(value)) if value.as_u64().is_some_and(|value| value > 0) => {
                false
            }
            _ => {
                return Err(CodegenError::new(format!(
                    "field group `{group_id}` has invalid max_occurs"
                )));
            }
        };
        for member in required_string_array(object, "members", "field group")? {
            if !fields.contains_key(member) {
                return Err(CodegenError::new(format!(
                    "field group `{group_id}` contains missing field `{member}`"
                )));
            }
            if let Some(previous) = memberships.insert(member.to_owned(), group_id.to_owned()) {
                return Err(CodegenError::new(format!(
                    "field `{member}` belongs to both groups `{previous}` and `{group_id}`"
                )));
            }
            if is_unbounded {
                unbounded_members.insert(member.to_owned());
            }
        }
    }
    for (field, declared_group) in fields {
        if declared_group.as_deref() != memberships.get(&field).map(String::as_str) {
            return Err(CodegenError::new(format!(
                "field `{field}` group_id and field-group membership disagree"
            )));
        }
    }
    Ok(unbounded_members.len())
}

fn validate_serialized_occurrences(document: &RuleSetDocument) -> Result<()> {
    let mut occurrences = BTreeSet::new();
    for field in &document.fields {
        let object = required_object(field, "field")?;
        let field_id = required_string(object, "field_id", "field")?;
        let serialized = match object.get("serialized") {
            Some(JsonValue::Array(values)) => values,
            _ => {
                return Err(CodegenError::new(format!(
                    "field `{field_id}` has invalid serialized array"
                )));
            }
        };
        for occurrence in serialized {
            let occurrence = required_object(occurrence, "serialized occurrence")?;
            let key = required_string(occurrence, "serialized_key", "serialized occurrence")?;
            let document_kind = required_string(occurrence, "document", "serialized occurrence")?;
            let number = required_u64(occurrence, "occurrence", "serialized occurrence")?;
            let identity = (document_kind.to_owned(), key.to_owned(), number);
            if !occurrences.insert(identity) {
                return Err(CodegenError::new(format!(
                    "duplicate serialized occurrence `{document_kind}:{key}:{number}`"
                )));
            }
        }
    }
    Ok(())
}

fn validate_rule_order(document: &RuleSetDocument) -> Result<()> {
    let mut phase_orders = BTreeMap::<(u8, u64), String>::new();
    let mut previous_key = None::<(u8, u64, String)>;
    for rule in &document.rules {
        let object = required_object(rule, "rule")?;
        let id = required_string(object, "rule_id", "rule")?;
        let order = required_u64(object, "order", "rule")?;
        let phases = required_string_array(object, "phases", "rule")?;
        let primary_phase = phases
            .iter()
            .map(|phase| validation_phase_rank(phase))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .min()
            .ok_or_else(|| CodegenError::new(format!("rule `{id}` has no phases")))?;
        for phase in &phases {
            let phase_rank = validation_phase_rank(phase)?;
            if let Some(previous_rule) = phase_orders.insert((phase_rank, order), id.to_owned()) {
                return Err(CodegenError::new(format!(
                    "duplicate rule order {order} in phase `{phase}` for rules `{previous_rule}` and `{id}`"
                )));
            }
        }

        let physical_key = (primary_phase, order, id.to_owned());
        if previous_key
            .as_ref()
            .is_some_and(|previous| previous >= &physical_key)
        {
            return Err(CodegenError::new(format!(
                "rule `{id}` is not in canonical physical order by primary phase, order, and rule_id"
            )));
        }
        previous_key = Some(physical_key);
    }
    Ok(())
}

fn validation_phase_rank(phase: &str) -> Result<u8> {
    match phase {
        "input" => Ok(0),
        "blur-change" => Ok(1),
        "page-navigation" => Ok(2),
        "save" => Ok(3),
        "draft-preview" => Ok(4),
        "validate" => Ok(5),
        "final-copy" => Ok(6),
        "submit" => Ok(7),
        _ => Err(CodegenError::new(format!(
            "unknown validation phase `{phase}` after schema validation"
        ))),
    }
}

#[cfg(test)]
mod rule_order_contract_tests {
    use super::*;
    use serde_json::json;

    fn rule(rule_id: &str, order: u64, phases: &[&str]) -> JsonValue {
        serde_json::from_value(json!({
            "rule_id": rule_id,
            "order": order,
            "phases": phases,
        }))
        .expect("build synthetic rule")
    }

    fn executable_rule(rule_id: &str, order: u64, phases: &[&str], scope: JsonValue) -> JsonValue {
        serde_json::from_value(json!({
            "rule_id": rule_id,
            "order": order,
            "phases": phases,
            "scope": scope.into_serde(),
            "profiles": {
                "official": {"state": "executable"},
                "filing_safe": {"state": "executable"}
            }
        }))
        .expect("build synthetic executable rule")
    }

    fn document_with_rules(rules: Vec<JsonValue>) -> RuleSetDocument {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let rule_set_path =
            manifest_dir.join("../../rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json");
        let rule_set = parse_strict(
            &read_bytes(&rule_set_path).expect("read candidate rule set"),
            &rule_set_path,
        )
        .expect("parse candidate rule set");
        let mut document: RuleSetDocument =
            serde_json::from_value(rule_set.into_serde()).expect("deserialize candidate rule set");
        document.rules = rules;
        document
    }

    #[test]
    fn phase_local_rule_order_accepts_disjoint_save_and_validate_sequences() {
        let document = document_with_rules(vec![
            rule("save-a", 1, &["save"]),
            rule("save-b", 2, &["save"]),
            rule("save-c", 3, &["save"]),
            rule("validate-a", 1, &["validate"]),
            rule("validate-b", 2, &["validate"]),
            rule("validate-c", 3, &["validate"]),
            rule("validate-d", 4, &["validate"]),
            rule("validate-e", 5, &["validate"]),
            rule("validate-f", 6, &["validate"]),
        ]);

        validate_rule_order(&document)
            .expect("phase-local ordering permits numeric order reuse across disjoint phases");
    }

    #[test]
    fn phase_local_rule_order_rejects_same_phase_collisions() {
        let document = document_with_rules(vec![
            rule("validate-a", 1, &["validate"]),
            rule("validate-b", 1, &["validate"]),
        ]);

        let error =
            validate_rule_order(&document).expect_err("same-phase order collision must fail");
        assert!(error.message().contains("order 1"));
        assert!(error.message().contains("phase `validate`"));
        assert!(error.message().contains("validate-a"));
        assert!(error.message().contains("validate-b"));
    }

    #[test]
    fn phase_local_rule_order_rejects_overlapping_multi_phase_collisions() {
        let document = document_with_rules(vec![
            rule("save-and-validate", 1, &["save", "validate"]),
            rule("validate-only", 1, &["validate"]),
        ]);

        let error = validate_rule_order(&document)
            .expect_err("overlapping multi-phase order collision must fail");
        assert!(error.message().contains("order 1"));
        assert!(error.message().contains("phase `validate`"));
        assert!(error.message().contains("save-and-validate"));
        assert!(error.message().contains("validate-only"));
    }

    #[test]
    fn physical_rule_order_is_canonical_by_primary_phase_then_order_then_id() {
        let document = document_with_rules(vec![
            rule("validate-a", 1, &["validate"]),
            rule("save-a", 1, &["save"]),
        ]);

        let error =
            validate_rule_order(&document).expect_err("noncanonical physical order must fail");
        assert!(error.message().contains("canonical physical order"));
    }

    #[test]
    fn fixture_rule_expectations_sort_by_order_rule_id_and_instance() {
        let document = document_with_rules(vec![
            executable_rule(
                "later",
                2,
                &["validate"],
                serde_json::from_value(json!({"kind": "singleton"}))
                    .expect("build singleton scope"),
            ),
            executable_rule(
                "rows",
                1,
                &["validate"],
                serde_json::from_value(json!({"kind": "each-group", "group_id": "rows"}))
                    .expect("build group scope"),
            ),
        ]);
        let group_instances = vec![
            FixtureGroupInstance {
                group_id: "rows".to_owned(),
                instance_id: "row-z".to_owned(),
            },
            FixtureGroupInstance {
                group_id: "rows".to_owned(),
                instance_id: "row-a".to_owned(),
            },
        ];

        let expectations =
            executable_rule_expectations(&document, "official", "validate", &group_instances)
                .expect("build fixture expectations");
        let actual = expectations
            .iter()
            .map(|expectation| {
                (
                    expectation.order,
                    expectation.execution.rule_id.as_str(),
                    expectation
                        .execution
                        .instance
                        .as_ref()
                        .map(|instance| instance.instance_id.as_str()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![
                (1, "rows", Some("row-a")),
                (1, "rows", Some("row-z")),
                (2, "later", None),
            ]
        );
    }
}

fn validate_fixture_identity(
    index: &IndexSnapshot,
    document: &RuleSetDocument,
    fixtures: &BTreeMap<String, JsonValue>,
) -> Result<()> {
    let mut ids = BTreeSet::new();
    for (path, fixture) in fixtures {
        reject_machine_local_paths(fixture, &format!("fixture:{path}"))?;
        let object = required_object(fixture, "fixture")?;
        let fixture_id = required_string(object, "fixture_id", "fixture")?;
        if !ids.insert(fixture_id.to_owned()) {
            return Err(CodegenError::new(format!(
                "duplicate fixture_id `{fixture_id}`"
            )));
        }
        match required_string(object, "kind", "fixture")? {
            "evaluation" => {
                let input = required_object(
                    object
                        .get("input")
                        .ok_or_else(|| CodegenError::new("fixture missing input"))?,
                    "evaluation fixture input",
                )?;
                let rule_set = required_object(
                    input.get("rule_set").ok_or_else(|| {
                        CodegenError::new("evaluation fixture input missing rule_set")
                    })?,
                    "evaluation fixture input rule_set",
                )?;
                let rule_set_id =
                    required_string(rule_set, "rule_set_id", "evaluation fixture input rule_set")?;
                if rule_set_id != index.rule_set_id {
                    return Err(CodegenError::new(format!(
                        "fixture `{fixture_id}` targets `{rule_set_id}`, expected `{}`",
                        index.rule_set_id
                    )));
                }
                validate_evaluation_fixture_identity(
                    fixture_id,
                    input,
                    object,
                    &document.identity,
                )?;
            }
            "compile-rejection" => {
                if document.review_status == ReviewStatus::Reviewed {
                    return Err(CodegenError::new(format!(
                        "reviewed snapshot cannot retain compile-rejection fixture `{fixture_id}`"
                    )));
                }
            }
            other => {
                return Err(CodegenError::new(format!(
                    "fixture `{fixture_id}` has unsupported kind `{other}`"
                )));
            }
        }
    }
    Ok(())
}

fn validate_reviewed_completeness(
    document: &RuleSetDocument,
    fixtures: &BTreeMap<String, JsonValue>,
    rules_root: &Path,
) -> Result<()> {
    validate_reviewed_typed_counts(document)?;
    validate_reviewed_legacy_mappings(document)?;
    let artifacts = load_reviewed_legacy_artifacts(document, rules_root)?;
    validate_reviewed_completeness_with_artifacts(document, fixtures, &artifacts)?;
    let negative_fixtures =
        load_reviewed_fixture_evidence(document, rules_root, "legacy-v1-negative-fixtures")?;
    validate_reviewed_legacy_negative_fixture_bijection(document, fixtures, &negative_fixtures)?;
    let calculation_fixtures =
        load_reviewed_fixture_evidence(document, rules_root, "legacy-v1-calculation-fixtures")?;
    validate_reviewed_legacy_calculation_fixture_coverage(document, fixtures, &calculation_fixtures)
}

pub(crate) fn validate_reviewed_evaluation_policy(document: &RuleSetDocument) -> Result<()> {
    let official = document.evaluation_policy.official.state();
    let filing_safe = document.evaluation_policy.filing_safe.state();
    if official == BranchState::Executable && filing_safe == BranchState::Executable {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "reviewed snapshot `{}` must make both evaluation-policy profiles executable; official is {official:?}, filing_safe is {filing_safe:?}; canonical states are official=`{}`, filing_safe=`{}`",
            document.identity.rule_set_id,
            branch_state_label(&official),
            branch_state_label(&filing_safe),
        )))
    }
}

fn branch_state_label(state: &BranchState) -> &'static str {
    match state {
        BranchState::Executable => "executable",
        BranchState::DocumentedOnly => "documented_only",
        BranchState::Unresolved => "unresolved",
    }
}

pub(crate) fn validate_candidate_readiness(
    document: &RuleSetDocument,
    fixtures: &BTreeMap<String, JsonValue>,
) -> Result<()> {
    let rule_set_id = &document.identity.rule_set_id;
    if document.identity.source_set_sha256.is_none() {
        return Err(CodegenError::new(format!(
            "candidate snapshot `{rule_set_id}` must pin source_set_sha256"
        )));
    }

    let official_executable = document.profile_status.official.state() == BranchState::Executable
        && document.evaluation_policy.official.state() == BranchState::Executable;
    let filing_safe_executable = document.profile_status.filing_safe.state()
        == BranchState::Executable
        && document.evaluation_policy.filing_safe.state() == BranchState::Executable;
    if !official_executable && !filing_safe_executable {
        return Err(CodegenError::new(format!(
            "candidate snapshot `{rule_set_id}` must have at least one profile whose rule-set and evaluation-policy branches are both executable"
        )));
    }
    if fixtures.is_empty() {
        return Err(CodegenError::new(format!(
            "candidate snapshot `{rule_set_id}` must have at least one concrete evaluation fixture"
        )));
    }

    for (path, fixture) in fixtures {
        let fixture = required_object(fixture, "candidate fixture")?;
        let fixture_id = required_string(fixture, "fixture_id", "candidate fixture")?;
        let kind = required_string(fixture, "kind", "candidate fixture")?;
        if kind != "evaluation" {
            return Err(CodegenError::new(format!(
                "candidate snapshot `{rule_set_id}` fixture `{fixture_id}` at `{path}` has non-evaluation kind `{kind}`"
            )));
        }
        let input = required_object(
            fixture.get("input").ok_or_else(|| {
                CodegenError::new(format!(
                    "candidate evaluation fixture `{fixture_id}` at `{path}` has no input"
                ))
            })?,
            "candidate evaluation fixture input",
        )?;
        let context = required_object(
            input.get("context").ok_or_else(|| {
                CodegenError::new(format!(
                    "candidate evaluation fixture `{fixture_id}` at `{path}` has no context"
                ))
            })?,
            "candidate evaluation fixture context",
        )?;
        let profile = required_string(context, "profile", "candidate evaluation fixture context")?;
        let executable = match profile {
            "official" => official_executable,
            "filing_safe" => filing_safe_executable,
            _ => false,
        };
        if !executable {
            return Err(CodegenError::new(format!(
                "candidate evaluation fixture `{fixture_id}` at `{path}` selects non-executable profile/evaluation-policy branch `{profile}`; candidate branches never fall back"
            )));
        }
        let expected = required_object(
            fixture.get("expected").ok_or_else(|| {
                CodegenError::new(format!(
                    "candidate evaluation fixture `{fixture_id}` at `{path}` has no expected result"
                ))
            })?,
            "candidate evaluation fixture expected result",
        )?;
        let validated_input = validate_fixture_input(document, path, input)?;
        validate_fixture_canonical_inputs(document, path, expected, &validated_input.raw_fields)?;
    }
    validate_candidate_workflow_fixture_coverage(document, fixtures)?;
    Ok(())
}

fn validate_candidate_workflow_fixture_coverage(
    document: &RuleSetDocument,
    fixtures: &BTreeMap<String, JsonValue>,
) -> Result<()> {
    let workflow = required_object(&document.workflow, "candidate workflow")?;
    if workflow.get("state").is_some() {
        if fixtures.values().any(|fixture| {
            fixture
                .object()
                .is_some_and(|fixture| fixture.contains_key("workflow_transition"))
        }) {
            return Err(CodegenError::new(
                "candidate with non-executable workflow cannot carry workflow_transition fixtures",
            ));
        }
        return Ok(());
    }

    let transitions = required_array(workflow, "transitions", "candidate workflow")?;
    let mut required = BTreeSet::<(String, String, String, String)>::new();
    for transition in transitions {
        let transition = required_object(transition, "candidate workflow transition")?;
        let transition_id =
            required_string(transition, "transition_id", "candidate workflow transition")?;
        let from_state =
            required_string(transition, "from_state", "candidate workflow transition")?;
        let action = required_string(transition, "action", "candidate workflow transition")?;
        let profiles = required_object(
            transition.get("profiles").ok_or_else(|| {
                CodegenError::new("candidate workflow transition is missing profiles")
            })?,
            "candidate workflow transition profiles",
        )?;
        for profile in ["official", "filing_safe"] {
            let branch = required_object(
                profiles.get(profile).ok_or_else(|| {
                    CodegenError::new("candidate workflow transition is missing profile")
                })?,
                "candidate workflow transition profile",
            )?;
            if branch.get("state").and_then(JsonValue::as_str) == Some("executable") {
                required.insert((
                    profile.to_owned(),
                    from_state.to_owned(),
                    action.to_owned(),
                    transition_id.to_owned(),
                ));
            }
        }
    }

    let mut covered = BTreeSet::new();
    for (path, fixture) in fixtures {
        let fixture = required_object(fixture, "candidate fixture")?;
        let Some(invocation) = fixture.get("workflow_transition") else {
            continue;
        };
        let invocation = required_object(
            invocation,
            "candidate workflow transition fixture invocation",
        )?;
        let input = required_object(
            fixture
                .get("input")
                .ok_or_else(|| CodegenError::new(format!("{path}: fixture missing input")))?,
            "candidate workflow transition fixture input",
        )?;
        let context = required_object(
            input
                .get("context")
                .ok_or_else(|| CodegenError::new(format!("{path}: input missing context")))?,
            "candidate workflow transition fixture context",
        )?;
        let profile = required_string(
            context,
            "profile",
            "candidate workflow transition fixture context",
        )?;
        let phase = required_string(
            context,
            "phase",
            "candidate workflow transition fixture context",
        )?;
        let current_state = required_string(
            invocation,
            "current_state",
            "candidate workflow transition fixture invocation",
        )?;
        let action = required_string(
            invocation,
            "action",
            "candidate workflow transition fixture invocation",
        )?;

        let matching = transitions
            .iter()
            .filter_map(JsonValue::object)
            .filter(|transition| {
                transition.get("from_state").and_then(JsonValue::as_str) == Some(current_state)
                    && transition.get("action").and_then(JsonValue::as_str) == Some(action)
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(CodegenError::new(format!(
                "{path}: workflow fixture selection matched {} transitions",
                matching.len()
            )));
        }
        let transition = matching[0];
        let transition_id =
            required_string(transition, "transition_id", "candidate workflow transition")?;
        let evaluation_phase = required_string(
            transition,
            "evaluation_phase",
            "candidate workflow transition",
        )?;
        if phase != evaluation_phase {
            return Err(CodegenError::new(format!(
                "{path}: workflow transition `{transition_id}` requires evaluated phase `{evaluation_phase}`, found `{phase}`"
            )));
        }
        let to_state = required_string(transition, "to_state", "candidate workflow transition")?;
        let profiles = required_object(
            transition
                .get("profiles")
                .ok_or_else(|| CodegenError::new("workflow transition missing profiles"))?,
            "candidate workflow transition profiles",
        )?;
        let branch = required_object(
            profiles
                .get(profile)
                .ok_or_else(|| CodegenError::new(format!("{path}: unknown profile `{profile}`")))?,
            "candidate workflow transition profile",
        )?;
        if branch.get("state").and_then(JsonValue::as_str) != Some("executable") {
            return Err(CodegenError::new(format!(
                "{path}: workflow transition `{transition_id}` profile `{profile}` is not executable"
            )));
        }

        let evaluation_expected = required_object(
            fixture
                .get("expected")
                .ok_or_else(|| CodegenError::new(format!("{path}: fixture missing expected")))?,
            "candidate workflow transition evaluation result",
        )?;
        let report = required_object(
            evaluation_expected.get("report").ok_or_else(|| {
                CodegenError::new(format!("{path}: expected evaluation missing report"))
            })?,
            "candidate workflow transition evaluation report",
        )?;
        if !required_array(
            report,
            "violations",
            "candidate workflow transition evaluation report",
        )?
        .is_empty()
        {
            return Err(CodegenError::new(format!(
                "{path}: workflow transition fixture requires a zero-violation evaluation"
            )));
        }

        let expected = required_object(
            invocation.get("expected").ok_or_else(|| {
                CodegenError::new(format!("{path}: workflow invocation missing expected"))
            })?,
            "candidate workflow transition expected result",
        )?;
        for (key, expected_value) in [
            ("transition_id", transition_id),
            ("from_state", current_state),
            ("action", action),
            ("to_state", to_state),
        ] {
            let actual = required_string(
                expected,
                key,
                "candidate workflow transition expected result",
            )?;
            if actual != expected_value {
                return Err(CodegenError::new(format!(
                    "{path}: workflow expected {key} `{actual}` differs from `{expected_value}`"
                )));
            }
        }
        for key in ["context", "input_revision", "context_fingerprint"] {
            if expected.get(key) != input.get(key) {
                return Err(CodegenError::new(format!(
                    "{path}: workflow expected {key} differs from evaluation input"
                )));
            }
        }
        if expected.get("rule_set") != input.get("rule_set") {
            return Err(CodegenError::new(format!(
                "{path}: workflow expected rule_set differs from evaluation input"
            )));
        }

        let expected_notifications = required_array(
            expected,
            "notifications",
            "candidate workflow transition expected result",
        )?;
        let effects = required_array(
            branch,
            "effects",
            "candidate workflow transition executable branch",
        )?;
        let source_notifications = effects
            .iter()
            .filter(|effect| {
                effect
                    .object()
                    .and_then(|effect| effect.get("kind"))
                    .and_then(JsonValue::as_str)
                    == Some("emit-notification")
            })
            .collect::<Vec<_>>();
        let notification_mismatch = expected_notifications.len() != source_notifications.len()
            || expected_notifications
                .iter()
                .zip(source_notifications)
                .any(|(expected, source)| {
                    let Some(expected) = expected.object() else {
                        return true;
                    };
                    let Some(source) = source.object() else {
                        return true;
                    };
                    ["channel", "message", "official_message"]
                        .iter()
                        .any(|key| expected.get(*key) != source.get(*key))
                });
        if notification_mismatch {
            return Err(CodegenError::new(format!(
                "{path}: workflow expected notifications do not exactly match transition effects"
            )));
        }
        covered.insert((
            profile.to_owned(),
            current_state.to_owned(),
            action.to_owned(),
            transition_id.to_owned(),
        ));
    }
    if covered != required {
        return Err(ordered_set_mismatch(
            "candidate executable workflow transition fixture coverage",
            &required,
            &covered,
        ));
    }
    Ok(())
}

fn validate_reviewed_completeness_with_artifacts(
    document: &RuleSetDocument,
    fixtures: &BTreeMap<String, JsonValue>,
    artifacts: &BTreeMap<LegacyArtifact, JsonValue>,
) -> Result<()> {
    validate_reviewed_evaluation_policy(document)?;
    validate_reviewed_legacy_manifest_binding(document, artifacts)?;
    validate_reviewed_legacy_locator_bijections(document, artifacts)?;
    validate_reviewed_fixture_coverage(document, fixtures)?;
    validate_candidate_workflow_fixture_coverage(document, fixtures)
}

fn validate_reviewed_typed_counts(document: &RuleSetDocument) -> Result<()> {
    let counts = &document.legacy_v1.declared_counts;
    if counts.unverified_gaps != 0 {
        return Err(CodegenError::new(format!(
            "reviewed snapshot must declare zero unverified legacy gaps, found {}",
            counts.unverified_gaps
        )));
    }
    validate_exact_count("typed fields", document.fields.len(), counts.typed_fields)?;
    validate_exact_count(
        "unbounded-family member fields",
        validated_unbounded_family_member_count(document)?,
        counts.unbounded_family_members,
    )?;
    validate_exact_count(
        "validation rules",
        document.rules.len(),
        counts.validation_rules,
    )?;
    validate_exact_count(
        "calculations",
        document.calculations.len(),
        counts.calculations,
    )?;

    let workflow = required_object(&document.workflow, "reviewed workflow")?;
    if workflow.get("state").is_some() {
        return Err(CodegenError::new(
            "reviewed workflow must be a typed workflow, not a non-executable branch",
        ));
    }
    let workflow_states = required_array(workflow, "states", "reviewed workflow")?;
    let workflow_transitions = required_array(workflow, "transitions", "reviewed workflow")?;
    validate_exact_count(
        "workflow states",
        workflow_states.len(),
        counts.workflow_states,
    )?;
    validate_exact_count(
        "workflow transitions",
        workflow_transitions.len(),
        counts.workflow_transitions,
    )?;

    let concrete_serialized_fields = document
        .fields
        .iter()
        .map(|field| {
            let field = required_object(field, "field")?;
            let serialized = required_array(field, "serialized", "field")?;
            serialized.iter().try_fold(false, |concrete, occurrence| {
                let occurrence = required_object(occurrence, "serialized occurrence")?;
                let presence = required_string(occurrence, "presence", "serialized occurrence")?;
                Ok(concrete || presence != "omitted")
            })
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|concrete| *concrete)
        .count();
    validate_exact_count(
        "concrete serialized fields",
        concrete_serialized_fields,
        counts.concrete_union_fields,
    )
}

fn validate_exact_count(label: &str, actual: usize, expected: u64) -> Result<()> {
    if actual as u64 == expected {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "reviewed {label} count {actual} differs from legacy declared count {expected}"
        )))
    }
}

fn validate_reviewed_legacy_mappings(document: &RuleSetDocument) -> Result<()> {
    for mapping in &document.legacy_v1.mappings {
        if mapping.state != BranchState::Executable {
            return Err(CodegenError::new(format!(
                "reviewed legacy {} mapping must be executable, found {:?}",
                mapping.artifact.label(),
                mapping.state
            )));
        }
    }
    Ok(())
}

fn load_reviewed_legacy_artifacts(
    document: &RuleSetDocument,
    rules_root: &Path,
) -> Result<BTreeMap<LegacyArtifact, JsonValue>> {
    let sources = document
        .sources
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut artifacts = BTreeMap::new();
    for artifact in [
        LegacyArtifact::Manifest,
        LegacyArtifact::Fields,
        LegacyArtifact::Validations,
        LegacyArtifact::Calculations,
        LegacyArtifact::Workflow,
    ] {
        let mapping = document
            .legacy_v1
            .mappings
            .iter()
            .find(|mapping| mapping.artifact == artifact)
            .expect("legacy artifact coverage was validated");
        let source = sources.get(mapping.source_id.as_str()).ok_or_else(|| {
            CodegenError::new(format!(
                "reviewed legacy {} mapping references missing source `{}`",
                artifact.label(),
                mapping.source_id
            ))
        })?;
        let path = resolve_existing_under(rules_root, &source.path, "reviewed legacy source path")?;
        artifacts.insert(artifact, parse_strict(&read_bytes(&path)?, &path)?);
    }
    Ok(artifacts)
}

#[derive(Clone, Debug)]
struct ReviewedFixtureEvidence {
    source_id: String,
    document: JsonValue,
}

fn load_reviewed_fixture_evidence(
    document: &RuleSetDocument,
    rules_root: &Path,
    source_kind: &str,
) -> Result<ReviewedFixtureEvidence> {
    let matching = document
        .sources
        .iter()
        .filter(|source| source.kind == source_kind)
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(CodegenError::new(format!(
            "reviewed snapshot must declare exactly one `{source_kind}` source, found {}",
            matching.len()
        )));
    }
    let source = matching[0];
    let path = resolve_existing_under(
        rules_root,
        &source.path,
        "reviewed legacy fixture source path",
    )?;
    Ok(ReviewedFixtureEvidence {
        source_id: source.source_id.clone(),
        document: parse_strict(&read_bytes(&path)?, &path)?,
    })
}

fn validate_reviewed_legacy_manifest_binding(
    document: &RuleSetDocument,
    artifacts: &BTreeMap<LegacyArtifact, JsonValue>,
) -> Result<()> {
    let manifest = artifacts
        .get(&LegacyArtifact::Manifest)
        .ok_or_else(|| CodegenError::new("reviewed legacy manifest artifact was not loaded"))?;
    let manifest = required_object(manifest, "reviewed legacy manifest")?;

    for (key, expected) in [
        ("schema_version", document.legacy_v1.schema_version.as_str()),
        ("form_id", document.legacy_v1.form_id.as_str()),
        ("form_code", document.identity.form_code.as_str()),
        ("revision", document.identity.form_revision.as_str()),
        (
            "package_version",
            document.identity.official_package_version.as_str(),
        ),
    ] {
        let actual = required_string(manifest, key, "reviewed legacy manifest")?;
        if actual != expected {
            return Err(CodegenError::new(format!(
                "reviewed legacy manifest {key} `{actual}` differs from v2 snapshot `{expected}`"
            )));
        }
    }
    let status = required_string(manifest, "status", "reviewed legacy manifest")?;
    if status != "complete" {
        return Err(CodegenError::new(format!(
            "reviewed legacy manifest status must be `complete`, found `{status}`"
        )));
    }

    let manifest_counts = required_object(
        manifest
            .get("counts")
            .ok_or_else(|| CodegenError::new("reviewed legacy manifest missing counts"))?,
        "reviewed legacy manifest counts",
    )?;
    let declared = &document.legacy_v1.declared_counts;
    for (key, expected) in [
        ("unbounded_families", declared.unbounded_family_members),
        ("typed_fields", declared.typed_fields),
        ("concrete_union_fields", declared.concrete_union_fields),
        ("validation_rules", declared.validation_rules),
        ("calculations", declared.calculations),
        ("negative_fixtures", declared.negative_fixtures),
        ("confirmed_official_bugs", declared.confirmed_official_bugs),
        ("unverified_gaps", declared.unverified_gaps),
    ] {
        let actual = required_u64(manifest_counts, key, "reviewed legacy manifest counts")?;
        if actual != expected {
            return Err(CodegenError::new(format!(
                "reviewed legacy manifest count `{key}` {actual} differs from v2 declared count {expected}"
            )));
        }
    }
    Ok(())
}

fn validate_reviewed_legacy_locator_bijections(
    document: &RuleSetDocument,
    artifacts: &BTreeMap<LegacyArtifact, JsonValue>,
) -> Result<()> {
    validate_legacy_entity_locator_bijection(
        document,
        LegacyArtifact::Fields,
        "fields",
        &document.fields,
        artifacts,
    )?;
    validate_legacy_entity_locator_bijection(
        document,
        LegacyArtifact::Validations,
        "rules",
        &document.rules,
        artifacts,
    )?;
    validate_legacy_entity_locator_bijection(
        document,
        LegacyArtifact::Calculations,
        "calculations",
        &document.calculations,
        artifacts,
    )?;

    let workflow = required_object(&document.workflow, "reviewed workflow")?;
    let states = required_array(workflow, "states", "reviewed workflow")?;
    validate_legacy_entity_locator_bijection(
        document,
        LegacyArtifact::Workflow,
        "phases",
        states,
        artifacts,
    )?;
    let transitions = required_array(workflow, "transitions", "reviewed workflow")?;
    validate_legacy_entity_locator_bijection(
        document,
        LegacyArtifact::Workflow,
        "transitions",
        transitions,
        artifacts,
    )
}

fn validate_legacy_entity_locator_bijection(
    document: &RuleSetDocument,
    artifact: LegacyArtifact,
    legacy_array_key: &str,
    typed_entities: &[JsonValue],
    artifacts: &BTreeMap<LegacyArtifact, JsonValue>,
) -> Result<()> {
    let mapping = document
        .legacy_v1
        .mappings
        .iter()
        .find(|mapping| mapping.artifact == artifact)
        .expect("legacy artifact coverage was validated");
    let legacy = artifacts.get(&artifact).ok_or_else(|| {
        CodegenError::new(format!(
            "reviewed legacy {} artifact was not loaded",
            artifact.label()
        ))
    })?;
    let legacy = required_object(legacy, "reviewed legacy artifact")?;
    let legacy_records = required_array(legacy, legacy_array_key, "reviewed legacy artifact")?;
    let mut covered = BTreeSet::new();

    for (typed_index, entity) in typed_entities.iter().enumerate() {
        let entity = required_object(entity, "reviewed typed entity")?;
        let source_refs = required_array(entity, "source_refs", "reviewed typed entity")?;
        let matching = source_refs
            .iter()
            .filter_map(|source_ref| {
                let source_ref = source_ref.object()?;
                (source_ref.get("source_id").and_then(JsonValue::as_str)
                    == Some(mapping.source_id.as_str()))
                .then_some(source_ref)
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(CodegenError::new(format!(
                "reviewed typed {legacy_array_key}[{typed_index}] must cite exactly one `{}` legacy source record, found {}",
                mapping.source_id,
                matching.len()
            )));
        }
        let locator = required_string(
            matching[0],
            "locator",
            "reviewed mapped legacy source reference",
        )?;
        let legacy_index = parse_canonical_legacy_array_locator(locator, legacy_array_key)?;
        if legacy_records.get(legacy_index).is_none() {
            return Err(CodegenError::new(format!(
                "reviewed typed {legacy_array_key}[{typed_index}] locator `{locator}` does not resolve"
            )));
        }
        if !covered.insert(legacy_index) {
            return Err(CodegenError::new(format!(
                "reviewed legacy locator `{locator}` is referenced more than once"
            )));
        }
    }

    let expected = (0..legacy_records.len()).collect::<BTreeSet<_>>();
    if covered != expected {
        let missing = expected.difference(&covered).copied().collect::<Vec<_>>();
        let extra = covered.difference(&expected).copied().collect::<Vec<_>>();
        return Err(CodegenError::new(format!(
            "reviewed legacy `{legacy_array_key}` locator bijection mismatch; missing indexes: {missing:?}; extra indexes: {extra:?}"
        )));
    }
    Ok(())
}

fn parse_canonical_legacy_array_locator(locator: &str, array_key: &str) -> Result<usize> {
    let prefix = format!("#/{array_key}/");
    let index = locator.strip_prefix(&prefix).ok_or_else(|| {
        CodegenError::new(format!(
            "legacy locator `{locator}` must use canonical JSON Pointer `{prefix}N`"
        ))
    })?;
    if index.is_empty() || index.contains('/') {
        return Err(CodegenError::new(format!(
            "legacy locator `{locator}` must identify one `{array_key}` array element"
        )));
    }
    let parsed = index.parse::<usize>().map_err(|_| {
        CodegenError::new(format!(
            "legacy locator `{locator}` has invalid array index `{index}`"
        ))
    })?;
    if parsed.to_string() != index {
        return Err(CodegenError::new(format!(
            "legacy locator `{locator}` has non-canonical array index `{index}`"
        )));
    }
    Ok(parsed)
}

fn validate_reviewed_legacy_negative_fixture_bijection(
    document: &RuleSetDocument,
    fixtures: &BTreeMap<String, JsonValue>,
    evidence: &ReviewedFixtureEvidence,
) -> Result<()> {
    let evidence_object = required_object(&evidence.document, "legacy negative-fixture evidence")?;
    validate_fixture_evidence_identity(document, evidence_object, "negative-fixture")?;
    let cases = required_array(evidence_object, "cases", "legacy negative-fixture evidence")?;
    validate_exact_count(
        "legacy negative fixtures",
        cases.len(),
        document.legacy_v1.declared_counts.negative_fixtures,
    )?;

    let mut case_ids = BTreeSet::new();
    for (index, case) in cases.iter().enumerate() {
        let case = required_object(case, "legacy negative fixture case")?;
        let case_id = required_string(case, "case_id", "legacy negative fixture case")?;
        if !case_ids.insert(case_id.to_owned()) {
            return Err(CodegenError::new(format!(
                "legacy negative fixture case_id `{case_id}` is duplicated"
            )));
        }
        required_string(case, "phase", "legacy negative fixture case")?;
        required_string(case, "rule_id", "legacy negative fixture case")?;
        required_string(case, "expected_message", "legacy negative fixture case")?;
        if case.get("mutations").is_none() {
            return Err(CodegenError::new(format!(
                "legacy negative fixture case `{case_id}` at index {index} has no mutations"
            )));
        }
    }

    let mut covered = BTreeSet::new();
    for (path, fixture) in fixtures {
        let fixture = required_object(fixture, "evaluation fixture")?;
        if required_string(fixture, "kind", "fixture")? != "evaluation" {
            continue;
        }
        let input = required_object(
            fixture
                .get("input")
                .ok_or_else(|| CodegenError::new(format!("{path}: fixture missing input")))?,
            "evaluation fixture input",
        )?;
        let context = required_object(
            input
                .get("context")
                .ok_or_else(|| CodegenError::new(format!("{path}: input missing context")))?,
            "evaluation fixture input context",
        )?;
        if required_string(context, "profile", "evaluation fixture input context")? != "official" {
            continue;
        }

        let expected = required_object(
            fixture
                .get("expected")
                .ok_or_else(|| CodegenError::new(format!("{path}: fixture missing expected")))?,
            "evaluation fixture expected result",
        )?;
        let report = required_object(
            expected.get("report").ok_or_else(|| {
                CodegenError::new(format!("{path}: expected result missing report"))
            })?,
            "evaluation fixture expected report",
        )?;
        let violations =
            required_array(report, "violations", "evaluation fixture expected report")?;
        if violations.is_empty() {
            continue;
        }

        let matching = fixture_source_locators(fixture, &evidence.source_id)?;
        if matching.len() > 1 {
            return Err(CodegenError::new(format!(
                "{path}: official negative fixture cites `{}` more than once",
                evidence.source_id
            )));
        }
        let Some(locator) = matching.first() else {
            continue;
        };
        let case_index = parse_canonical_legacy_array_locator(locator, "cases")?;
        let case = cases.get(case_index).ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: legacy negative fixture locator `{locator}` does not resolve"
            ))
        })?;
        if !covered.insert(case_index) {
            return Err(CodegenError::new(format!(
                "legacy negative fixture locator `{locator}` is covered more than once"
            )));
        }
        if violations.len() != 1 {
            return Err(CodegenError::new(format!(
                "{path}: fixture translating `{locator}` must isolate exactly one official violation"
            )));
        }

        let case = required_object(case, "legacy negative fixture case")?;
        let violation = required_object(&violations[0], "evaluation fixture violation")?;
        let expected_rule = required_string(case, "rule_id", "legacy negative fixture case")?;
        let execution = required_object(
            violation.get("execution").ok_or_else(|| {
                CodegenError::new(format!(
                    "{path}: evaluation fixture violation missing execution"
                ))
            })?,
            "evaluation fixture violation execution",
        )?;
        let actual_rule = required_string(
            execution,
            "rule_id",
            "evaluation fixture violation execution",
        )?;
        if actual_rule != expected_rule {
            return Err(CodegenError::new(format!(
                "{path}: translated legacy negative case `{locator}` expects rule `{expected_rule}`, found `{actual_rule}`"
            )));
        }
        let legacy_phase = required_string(case, "phase", "legacy negative fixture case")?;
        let expected_phase = canonical_legacy_phase(legacy_phase)?;
        let actual_phase = required_string(context, "phase", "evaluation fixture input context")?;
        if actual_phase != expected_phase {
            return Err(CodegenError::new(format!(
                "{path}: translated legacy negative case `{locator}` expects phase `{expected_phase}`, found `{actual_phase}`"
            )));
        }
        let expected_message =
            required_string(case, "expected_message", "legacy negative fixture case")?;
        let selected_message =
            required_string(violation, "message", "evaluation fixture violation")?;
        let official_message = required_string(
            violation,
            "official_message",
            "evaluation fixture violation",
        )?;
        if selected_message != expected_message || official_message != expected_message {
            return Err(CodegenError::new(format!(
                "{path}: translated legacy negative case `{locator}` does not preserve exact official message"
            )));
        }
    }

    let expected = (0..cases.len()).collect::<BTreeSet<_>>();
    if covered != expected {
        let expected_locators = expected
            .iter()
            .map(|index| format!("#/cases/{index}"))
            .collect::<BTreeSet<_>>();
        let covered_locators = covered
            .iter()
            .map(|index| format!("#/cases/{index}"))
            .collect::<BTreeSet<_>>();
        return Err(set_mismatch(
            "reviewed official legacy negative-fixture locator coverage",
            &expected_locators,
            &covered_locators,
        ));
    }
    Ok(())
}

fn validate_reviewed_legacy_calculation_fixture_coverage(
    document: &RuleSetDocument,
    fixtures: &BTreeMap<String, JsonValue>,
    evidence: &ReviewedFixtureEvidence,
) -> Result<()> {
    let evidence_object =
        required_object(&evidence.document, "legacy calculation-fixture evidence")?;
    validate_fixture_evidence_identity(document, evidence_object, "calculation-fixture")?;
    let cases = required_array(
        evidence_object,
        "cases",
        "legacy calculation-fixture evidence",
    )?;
    validate_exact_count(
        "legacy calculation fixtures",
        cases.len(),
        document.legacy_v1.declared_counts.calculations,
    )?;

    let executable_calculations = collect_object_ids(&document.calculations, "calculation_id")?;
    let mut case_calculations = BTreeSet::new();
    for case in cases {
        let case = required_object(case, "legacy calculation fixture case")?;
        let calculation_id =
            required_string(case, "calculation_id", "legacy calculation fixture case")?;
        if !executable_calculations.contains(calculation_id) {
            return Err(CodegenError::new(format!(
                "legacy calculation fixture references missing calculation `{calculation_id}`"
            )));
        }
        if !case_calculations.insert(calculation_id.to_owned()) {
            return Err(CodegenError::new(format!(
                "legacy calculation fixture repeats calculation `{calculation_id}`"
            )));
        }
    }
    let expected_calculations = executable_calculations
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<BTreeSet<_>>();
    if case_calculations != expected_calculations {
        return Err(set_mismatch(
            "legacy calculation-fixture calculation coverage",
            &expected_calculations,
            &case_calculations,
        ));
    }

    let mut covered = BTreeSet::new();
    for (path, fixture) in fixtures {
        let fixture = required_object(fixture, "evaluation fixture")?;
        if required_string(fixture, "kind", "fixture")? != "evaluation" {
            continue;
        }
        let expected = required_object(
            fixture
                .get("expected")
                .ok_or_else(|| CodegenError::new(format!("{path}: fixture missing expected")))?,
            "evaluation fixture expected result",
        )?;
        let fixture_outputs = fixture_output_expectations(path, expected, "expected_outputs")?;
        for locator in fixture_source_locators(fixture, &evidence.source_id)? {
            let case_index = parse_canonical_legacy_array_locator(locator, "cases")?;
            let case = cases.get(case_index).ok_or_else(|| {
                CodegenError::new(format!(
                    "{path}: legacy calculation fixture locator `{locator}` does not resolve"
                ))
            })?;
            let case = required_object(case, "legacy calculation fixture case")?;
            let calculation_id =
                required_string(case, "calculation_id", "legacy calculation fixture case")?;
            if !fixture_outputs
                .iter()
                .any(|output| output.calculation_id == calculation_id)
            {
                return Err(CodegenError::new(format!(
                    "{path}: fixture cites calculation case `{locator}` but expects no output from `{calculation_id}`"
                )));
            }
            covered.insert(case_index);
        }
    }
    let expected = (0..cases.len()).collect::<BTreeSet<_>>();
    if covered != expected {
        let expected_locators = expected
            .iter()
            .map(|index| format!("#/cases/{index}"))
            .collect::<BTreeSet<_>>();
        let covered_locators = covered
            .iter()
            .map(|index| format!("#/cases/{index}"))
            .collect::<BTreeSet<_>>();
        return Err(set_mismatch(
            "reviewed legacy calculation-fixture locator coverage",
            &expected_locators,
            &covered_locators,
        ));
    }
    Ok(())
}

fn validate_fixture_evidence_identity(
    document: &RuleSetDocument,
    evidence: &BTreeMap<String, JsonValue>,
    label: &str,
) -> Result<()> {
    let schema_version = required_string(evidence, "schema_version", "legacy fixture evidence")?;
    if schema_version != document.legacy_v1.schema_version {
        return Err(CodegenError::new(format!(
            "legacy {label} schema_version `{schema_version}` differs from `{}`",
            document.legacy_v1.schema_version
        )));
    }
    let form_id = required_string(evidence, "form_id", "legacy fixture evidence")?;
    if form_id != document.legacy_v1.form_id {
        return Err(CodegenError::new(format!(
            "legacy {label} form_id `{form_id}` differs from `{}`",
            document.legacy_v1.form_id
        )));
    }
    Ok(())
}

fn fixture_source_locators<'a>(
    fixture: &'a BTreeMap<String, JsonValue>,
    source_id: &str,
) -> Result<Vec<&'a str>> {
    let source_refs = required_array(fixture, "source_refs", "evaluation fixture")?;
    source_refs
        .iter()
        .filter_map(|source_ref| {
            let source_ref = match required_object(source_ref, "evaluation fixture source_ref") {
                Ok(source_ref) => source_ref,
                Err(error) => return Some(Err(error)),
            };
            if source_ref.get("source_id").and_then(JsonValue::as_str) != Some(source_id) {
                return None;
            }
            Some(required_string(
                source_ref,
                "locator",
                "evaluation fixture source_ref",
            ))
        })
        .collect()
}

fn canonical_legacy_phase(phase: &str) -> Result<&'static str> {
    match phase {
        "input" => Ok("input"),
        "blur/change" => Ok("blur-change"),
        "page navigation" => Ok("page-navigation"),
        "save" => Ok("save"),
        "draft preview" => Ok("draft-preview"),
        "validate" => Ok("validate"),
        "final copy" => Ok("final-copy"),
        "submit" => Ok("submit"),
        other => Err(CodegenError::new(format!(
            "unsupported legacy negative-fixture phase `{other}`"
        ))),
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FixtureGroupInstance {
    group_id: String,
    instance_id: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FixtureFieldInstance {
    field_id: String,
    group_path: Vec<FixtureGroupInstance>,
}

#[derive(Clone, Debug)]
struct ValidatedFixtureInput {
    group_instances: Vec<FixtureGroupInstance>,
    raw_fields: BTreeMap<FixtureFieldInstance, JsonValue>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FixtureRuleExecution {
    rule_id: String,
    instance: Option<FixtureGroupInstance>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FixtureRuleExpectation {
    execution: FixtureRuleExecution,
    order: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FixtureOutputExpectation {
    calculation_id: String,
    output_id: String,
    instance: Option<FixtureGroupInstance>,
}

fn validate_reviewed_fixture_coverage(
    document: &RuleSetDocument,
    fixtures: &BTreeMap<String, JsonValue>,
) -> Result<()> {
    const PROFILES: [&str; 2] = ["official", "filing_safe"];

    let mut seen_profiles = BTreeSet::new();
    let mut covered_rules = BTreeMap::<String, BTreeSet<String>>::new();
    let mut covered_outputs = BTreeMap::<String, BTreeSet<FixtureOutputExpectation>>::new();
    let mut violated_rules = BTreeMap::<String, BTreeSet<String>>::new();
    let mut positive_fixture_counts = BTreeMap::<String, usize>::new();
    let mut negative_fixture_counts = BTreeMap::<String, usize>::new();

    for (path, fixture) in fixtures {
        let fixture = required_object(fixture, "fixture")?;
        if required_string(fixture, "kind", "fixture")? != "evaluation" {
            continue;
        }

        let input = required_object(
            fixture
                .get("input")
                .ok_or_else(|| CodegenError::new(format!("{path}: fixture missing input")))?,
            "evaluation fixture input",
        )?;
        let input_context = required_object(
            input
                .get("context")
                .ok_or_else(|| CodegenError::new(format!("{path}: input missing context")))?,
            "evaluation fixture input context",
        )?;
        let phase = required_string(input_context, "phase", "evaluation fixture input context")?;
        let profile =
            required_string(input_context, "profile", "evaluation fixture input context")?;
        if !PROFILES.contains(&profile) {
            return Err(CodegenError::new(format!(
                "{path}: unsupported evaluation fixture profile `{profile}`"
            )));
        }
        seen_profiles.insert(profile.to_owned());

        let expected = required_object(
            fixture
                .get("expected")
                .ok_or_else(|| CodegenError::new(format!("{path}: fixture missing expected")))?,
            "evaluation fixture expected result",
        )?;
        let report = required_object(
            expected.get("report").ok_or_else(|| {
                CodegenError::new(format!("{path}: expected result missing report"))
            })?,
            "evaluation fixture expected report",
        )?;
        validate_fixture_context(path, input, report)?;
        let validated_input = validate_fixture_input(document, path, input)?;
        validate_fixture_canonical_inputs(document, path, expected, &validated_input.raw_fields)?;

        let expected_rules = fixture_rule_expectations(path, report)?;
        let applicable_rules = executable_rule_expectations(
            document,
            profile,
            phase,
            &validated_input.group_instances,
        )?;
        if expected_rules != applicable_rules {
            return Err(CodegenError::new(format!(
                "{path}: expected_rules do not exactly match executable `{profile}` rules for phase `{phase}`; expected {applicable_rules:?}, found {expected_rules:?}"
            )));
        }
        validate_evaluated_rule_projection(path, report, &expected_rules)?;
        let fixture_violations =
            validate_violation_context(document, path, report, phase, profile, &expected_rules)?;
        let has_no_violations = fixture_violations.is_empty();
        if !has_no_violations {
            *negative_fixture_counts
                .entry(profile.to_owned())
                .or_default() += 1;
            violated_rules
                .entry(profile.to_owned())
                .or_default()
                .extend(fixture_violations);
        }
        covered_rules.entry(profile.to_owned()).or_default().extend(
            expected_rules
                .iter()
                .map(|rule| rule.execution.rule_id.clone()),
        );

        let fixture_outputs = fixture_output_expectations(path, expected, "expected_outputs")?;
        let applicable_outputs = executable_output_expectations(
            document,
            profile,
            phase,
            &validated_input.group_instances,
        )?;
        validate_exact_output_coverage(
            path,
            profile,
            phase,
            &applicable_outputs,
            &fixture_outputs,
        )?;
        let derived_outputs = fixture_output_expectations(path, expected, "derived_outputs")?;
        if derived_outputs != fixture_outputs {
            return Err(CodegenError::new(format!(
                "{path}: derived_outputs do not exactly project expected_outputs"
            )));
        }
        if has_no_violations && (!expected_rules.is_empty() || !fixture_outputs.is_empty()) {
            *positive_fixture_counts
                .entry(profile.to_owned())
                .or_default() += 1;
        }
        covered_outputs
            .entry(profile.to_owned())
            .or_default()
            .extend(fixture_outputs);
    }

    for profile in PROFILES {
        if !seen_profiles.contains(profile) {
            return Err(CodegenError::new(format!(
                "reviewed snapshot has no concrete evaluation fixture for profile `{profile}`"
            )));
        }
        if positive_fixture_counts.get(profile).copied().unwrap_or(0) == 0 {
            return Err(CodegenError::new(format!(
                "reviewed snapshot has no zero-violation positive evaluation fixture exercising an executable rule or output for profile `{profile}`"
            )));
        }
        let actual_negative_fixtures = negative_fixture_counts.get(profile).copied().unwrap_or(0);
        if profile == "official" {
            // The v1 fixture inventory is evidence for official-package
            // behavior, so its declared count binds only the official branch.
            let expected = document.legacy_v1.declared_counts.negative_fixtures;
            if (actual_negative_fixtures as u64) < expected {
                return Err(CodegenError::new(format!(
                    "reviewed `official` negative evaluation fixture count {actual_negative_fixtures} is below legacy declared official-package count {expected}"
                )));
            }
        } else if actual_negative_fixtures == 0 {
            return Err(CodegenError::new(
                "reviewed `filing_safe` profile must include at least one negative evaluation fixture",
            ));
        }

        let expected_rules = all_executable_rule_ids(document, profile)?;
        let actual_rules = covered_rules.remove(profile).unwrap_or_default();
        if actual_rules != expected_rules {
            return Err(set_mismatch(
                &format!("reviewed `{profile}` executable rule fixture coverage"),
                &expected_rules,
                &actual_rules,
            ));
        }
        let expected_violated_rules = all_executable_issue_rule_ids(document, profile)?;
        let actual_violated_rules = violated_rules.remove(profile).unwrap_or_default();
        if actual_violated_rules != expected_violated_rules {
            return Err(set_mismatch(
                &format!("reviewed `{profile}` issue-rule violation fixture coverage"),
                &expected_violated_rules,
                &actual_violated_rules,
            ));
        }

        let expected_outputs = all_executable_outputs(document, profile)?;
        let actual_outputs = covered_outputs
            .remove(profile)
            .unwrap_or_default()
            .into_iter()
            .map(|output| (output.calculation_id, output.output_id))
            .collect::<BTreeSet<_>>();
        if actual_outputs != expected_outputs {
            return Err(ordered_set_mismatch(
                &format!("reviewed `{profile}` executable output fixture coverage"),
                &expected_outputs,
                &actual_outputs,
            ));
        }
    }
    Ok(())
}

fn validate_evaluation_fixture_identity(
    fixture_id: &str,
    input: &BTreeMap<String, JsonValue>,
    fixture: &BTreeMap<String, JsonValue>,
    identity: &crate::model::RuleSetIdentity,
) -> Result<()> {
    let input_rule_set = required_object(
        input
            .get("rule_set")
            .ok_or_else(|| CodegenError::new("evaluation fixture input missing rule_set"))?,
        "evaluation fixture input rule_set",
    )?;
    validate_fixture_rule_set_identity(fixture_id, "input", input_rule_set, identity)?;

    let expected = required_object(
        fixture
            .get("expected")
            .ok_or_else(|| CodegenError::new("evaluation fixture missing expected result"))?,
        "evaluation fixture expected result",
    )?;
    let report = required_object(
        expected.get("report").ok_or_else(|| {
            CodegenError::new("evaluation fixture expected result missing report")
        })?,
        "evaluation fixture expected report",
    )?;
    let report_rule_set = required_object(
        report
            .get("rule_set")
            .ok_or_else(|| CodegenError::new("evaluation fixture report missing rule_set"))?,
        "evaluation fixture report rule_set",
    )?;
    validate_fixture_rule_set_identity(fixture_id, "expected report", report_rule_set, identity)
}

fn validate_fixture_rule_set_identity(
    fixture_id: &str,
    location: &str,
    actual: &BTreeMap<String, JsonValue>,
    expected: &crate::model::RuleSetIdentity,
) -> Result<()> {
    for (key, expected_value) in [
        ("rule_set_id", expected.rule_set_id.as_str()),
        ("form_code", expected.form_code.as_str()),
        ("form_revision", expected.form_revision.as_str()),
        (
            "official_package_version",
            expected.official_package_version.as_str(),
        ),
    ] {
        let actual_value = required_string(actual, key, "evaluation fixture rule-set identity")?;
        if actual_value != expected_value {
            return Err(CodegenError::new(format!(
                "fixture `{fixture_id}` {location} {key} `{actual_value}` differs from snapshot `{expected_value}`"
            )));
        }
    }
    if let Some(expected_digest) = expected.source_set_sha256.as_deref() {
        let actual_digest = required_string(
            actual,
            "source_set_sha256",
            "evaluation fixture rule-set identity",
        )?;
        if actual_digest != expected_digest {
            return Err(CodegenError::new(format!(
                "fixture `{fixture_id}` {location} source_set_sha256 `{actual_digest}` differs from snapshot `{expected_digest}`"
            )));
        }
    }
    Ok(())
}

fn validate_fixture_context(
    path: &str,
    input: &BTreeMap<String, JsonValue>,
    report: &BTreeMap<String, JsonValue>,
) -> Result<()> {
    let input_context = required_object(
        input
            .get("context")
            .ok_or_else(|| CodegenError::new(format!("{path}: input missing context")))?,
        "evaluation fixture input context",
    )?;
    let report_context = required_object(
        report
            .get("context")
            .ok_or_else(|| CodegenError::new(format!("{path}: report missing context")))?,
        "evaluation fixture report context",
    )?;
    for key in ["phase", "profile"] {
        let input = required_string(input_context, key, "evaluation fixture input context")?;
        let report = required_string(report_context, key, "evaluation fixture report context")?;
        if input != report {
            return Err(CodegenError::new(format!(
                "{path}: input context {key} `{input}` differs from report `{report}`"
            )));
        }
    }
    let input_revision = required_u64(input, "input_revision", "evaluation fixture input")?;
    let report_revision = required_u64(report, "input_revision", "evaluation fixture report")?;
    if input_revision != report_revision {
        return Err(CodegenError::new(format!(
            "{path}: input revision {input_revision} differs from report {report_revision}"
        )));
    }
    let input_fingerprint =
        required_string(input, "context_fingerprint", "evaluation fixture input")?;
    let report_fingerprint =
        required_string(report, "context_fingerprint", "evaluation fixture report")?;
    if input_fingerprint != report_fingerprint {
        return Err(CodegenError::new(format!(
            "{path}: input context_fingerprint `{input_fingerprint}` differs from report `{report_fingerprint}`"
        )));
    }
    Ok(())
}

fn validate_fixture_input(
    document: &RuleSetDocument,
    path: &str,
    input: &BTreeMap<String, JsonValue>,
) -> Result<ValidatedFixtureInput> {
    validate_fixture_context_values(document, path, input)?;
    let raw_inputs = required_object(
        input
            .get("raw_inputs")
            .ok_or_else(|| CodegenError::new(format!("{path}: input missing raw_inputs")))?,
        "evaluation fixture raw inputs",
    )?;
    let mut group_instances = required_array(
        raw_inputs,
        "repeated_group_instances",
        "evaluation fixture raw inputs",
    )?
    .iter()
    .map(|instance| fixture_group_instance(instance, "evaluation fixture group instance"))
    .collect::<Result<Vec<_>>>()?;
    group_instances.sort();
    for pair in group_instances.windows(2) {
        if pair[0] == pair[1] {
            return Err(CodegenError::new(format!(
                "{path}: duplicate repeated-group instance {}:{}",
                pair[0].group_id, pair[0].instance_id
            )));
        }
    }

    let mut group_bounds = BTreeMap::<String, (usize, Option<usize>)>::new();
    for group in &document.field_groups {
        let group = required_object(group, "field group")?;
        let group_id = required_string(group, "group_id", "field group")?.to_owned();
        let minimum =
            usize::try_from(required_u64(group, "min_occurs", "field group")?).map_err(|_| {
                CodegenError::new(format!("{path}: group `{group_id}` minimum exceeds usize"))
            })?;
        let maximum = match group.get("max_occurs") {
            Some(JsonValue::Null) => None,
            Some(JsonValue::Number(value)) => value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .map(Some)
                .ok_or_else(|| {
                    CodegenError::new(format!("{path}: group `{group_id}` maximum exceeds usize"))
                })?,
            _ => {
                return Err(CodegenError::new(format!(
                    "{path}: group `{group_id}` has invalid max_occurs"
                )));
            }
        };
        group_bounds.insert(group_id, (minimum, maximum));
    }
    for instance in &group_instances {
        if !group_bounds.contains_key(&instance.group_id) {
            return Err(CodegenError::new(format!(
                "{path}: repeated-group instance references undeclared group `{}`",
                instance.group_id
            )));
        }
    }
    for (group_id, (minimum, maximum)) in &group_bounds {
        let actual = group_instances
            .iter()
            .filter(|instance| &instance.group_id == group_id)
            .count();
        if actual < *minimum || maximum.is_some_and(|maximum| actual > maximum) {
            return Err(CodegenError::new(format!(
                "{path}: group `{group_id}` fixture cardinality {actual} is outside {minimum}..={maximum:?}"
            )));
        }
    }

    let mut field_groups = BTreeMap::<String, Option<String>>::new();
    for field in &document.fields {
        let field = required_object(field, "field")?;
        let field_id = required_string(field, "field_id", "field")?.to_owned();
        let group_id = match field.get("group_id") {
            Some(JsonValue::String(group_id)) => Some(group_id.clone()),
            Some(JsonValue::Null) => None,
            _ => {
                return Err(CodegenError::new(format!(
                    "{path}: field `{field_id}` has invalid group_id"
                )));
            }
        };
        field_groups.insert(field_id, group_id);
    }

    let mut actual_fields = Vec::new();
    for raw in required_array(raw_inputs, "fields", "evaluation fixture raw inputs")? {
        let raw = required_object(raw, "evaluation fixture raw field")?;
        let field = fixture_field_instance(
            raw.get("field").ok_or_else(|| {
                CodegenError::new(format!("{path}: raw field is missing field identity"))
            })?,
            "evaluation fixture raw field identity",
        )?;
        for instance in &field.group_path {
            if group_instances.binary_search(instance).is_err() {
                return Err(CodegenError::new(format!(
                    "{path}: raw field `{}` references undeclared instance {}:{}",
                    field.field_id, instance.group_id, instance.instance_id
                )));
            }
        }
        let declared_group = field_groups.get(&field.field_id).ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: raw field `{}` does not resolve",
                field.field_id
            ))
        })?;
        let valid_shape = match declared_group {
            None => field.group_path.is_empty(),
            Some(group_id) => {
                field.group_path.len() == 1 && field.group_path[0].group_id.as_str() == group_id
            }
        };
        if !valid_shape {
            return Err(CodegenError::new(format!(
                "{path}: raw field `{}` group path {:?} is incompatible with declared group {declared_group:?}",
                field.field_id, field.group_path
            )));
        }
        let value = raw
            .get("value")
            .ok_or_else(|| CodegenError::new(format!("{path}: raw field is missing value")))?
            .clone();
        actual_fields.push((field, value));
    }
    actual_fields.sort_by(|left, right| left.0.cmp(&right.0));
    for pair in actual_fields.windows(2) {
        if pair[0].0 == pair[1].0 {
            return Err(CodegenError::new(format!(
                "{path}: duplicate raw field identity {:?}",
                pair[0].0
            )));
        }
    }

    let mut expected_fields = Vec::new();
    for (field_id, group_id) in &field_groups {
        match group_id {
            None => expected_fields.push(FixtureFieldInstance {
                field_id: field_id.clone(),
                group_path: Vec::new(),
            }),
            Some(group_id) => {
                expected_fields.extend(
                    group_instances
                        .iter()
                        .filter(|instance| &instance.group_id == group_id)
                        .cloned()
                        .map(|instance| FixtureFieldInstance {
                            field_id: field_id.clone(),
                            group_path: vec![instance],
                        }),
                );
            }
        }
    }
    expected_fields.sort();
    let actual_field_identities = actual_fields
        .iter()
        .map(|(field, _)| field.clone())
        .collect::<Vec<_>>();
    if actual_field_identities != expected_fields {
        return Err(CodegenError::new(format!(
            "{path}: raw fixture field coverage mismatch; expected {expected_fields:?}, found {actual_field_identities:?}"
        )));
    }

    Ok(ValidatedFixtureInput {
        group_instances,
        raw_fields: actual_fields.into_iter().collect(),
    })
}

fn validate_fixture_canonical_inputs(
    document: &RuleSetDocument,
    path: &str,
    expected: &BTreeMap<String, JsonValue>,
    raw_fields: &BTreeMap<FixtureFieldInstance, JsonValue>,
) -> Result<()> {
    let mut field_types = BTreeMap::new();
    for field in &document.fields {
        let field = required_object(field, "field")?;
        let field_id = required_string(field, "field_id", "field")?;
        let value_type = required_string(field, "value_type", "field")?;
        field_types.insert(field_id, value_type);
    }

    let mut canonical_identities = Vec::new();
    for item in required_array(
        expected,
        "canonical_inputs",
        "evaluation fixture expected result",
    )? {
        let item = required_object(item, "evaluation fixture canonical input")?;
        let field = fixture_field_instance(
            item.get("field").ok_or_else(|| {
                CodegenError::new(format!("{path}: canonical input is missing field identity"))
            })?,
            "evaluation fixture canonical input field",
        )?;
        let expected_raw = raw_fields.get(&field).ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: canonical input references field identity {field:?} absent from raw_inputs"
            ))
        })?;
        let actual_raw = item.get("raw").ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: canonical input for {field:?} is missing raw"
            ))
        })?;
        if actual_raw != expected_raw {
            return Err(CodegenError::new(format!(
                "{path}: canonical input raw value for {field:?} does not echo raw_inputs"
            )));
        }
        let canonical = item.get("canonical").ok_or_else(|| {
            CodegenError::new(format!(
                "{path}: canonical input for {field:?} is missing canonical value"
            ))
        })?;
        let mut validated_wire = String::new();
        append_runtime_canonical_value_json(&mut validated_wire, canonical)?;
        if let Some(actual_type) = fixture_canonical_value_type(canonical, path)? {
            let expected_type = field_types.get(field.field_id.as_str()).ok_or_else(|| {
                CodegenError::new(format!(
                    "{path}: canonical input field `{}` does not resolve",
                    field.field_id
                ))
            })?;
            if actual_type != *expected_type {
                return Err(CodegenError::new(format!(
                    "{path}: canonical input field `{}` has type `{actual_type}`, expected `{expected_type}`",
                    field.field_id
                )));
            }
        }
        canonical_identities.push(field);
    }

    let expected_identities = raw_fields.keys().cloned().collect::<Vec<_>>();
    if canonical_identities != expected_identities {
        return Err(CodegenError::new(format!(
            "{path}: canonical_inputs must exactly cover raw_inputs in stable field order; expected {expected_identities:?}, found {canonical_identities:?}"
        )));
    }
    Ok(())
}

fn fixture_group_instance(value: &JsonValue, label: &str) -> Result<FixtureGroupInstance> {
    let instance = required_object(value, label)?;
    Ok(FixtureGroupInstance {
        group_id: required_string(instance, "group_id", label)?.to_owned(),
        instance_id: required_string(instance, "instance_id", label)?.to_owned(),
    })
}

fn fixture_field_instance(value: &JsonValue, label: &str) -> Result<FixtureFieldInstance> {
    let field = required_object(value, label)?;
    let field_id = required_string(field, "field_id", label)?.to_owned();
    let group_path = required_array(field, "group_path", label)?
        .iter()
        .map(|instance| fixture_group_instance(instance, label))
        .collect::<Result<Vec<_>>>()?;
    let mut group_ids = BTreeSet::new();
    for instance in &group_path {
        if !group_ids.insert(instance.group_id.as_str()) {
            return Err(CodegenError::new(format!(
                "{label}: field `{field_id}` repeats group `{}` in its path",
                instance.group_id
            )));
        }
    }
    Ok(FixtureFieldInstance {
        field_id,
        group_path,
    })
}

fn validate_fixture_context_values(
    document: &RuleSetDocument,
    path: &str,
    input: &BTreeMap<String, JsonValue>,
) -> Result<()> {
    let snapshot = required_object(
        input
            .get("context_values")
            .ok_or_else(|| CodegenError::new(format!("{path}: input missing context_values")))?,
        "evaluation fixture context snapshot",
    )?;
    let mut values = required_array(snapshot, "values", "evaluation fixture context snapshot")?
        .iter()
        .map(|value| {
            let value = required_object(value, "evaluation fixture context value")?;
            Ok((
                required_string(value, "id", "evaluation fixture context value")?.to_owned(),
                value
                    .get("value")
                    .ok_or_else(|| {
                        CodegenError::new(format!("{path}: context value is missing value"))
                    })?
                    .clone(),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    values.sort_by(|left, right| left.0.cmp(&right.0));
    for pair in values.windows(2) {
        if pair[0].0 == pair[1].0 {
            return Err(CodegenError::new(format!(
                "{path}: duplicate context value `{}`",
                pair[0].0
            )));
        }
    }

    let mut specifications = BTreeMap::<String, (String, bool)>::new();
    for specification in &document.context_values {
        let specification = required_object(specification, "context value specification")?;
        let id = required_string(
            specification,
            "context_value_id",
            "context value specification",
        )?;
        let value_type =
            required_string(specification, "value_type", "context value specification")?;
        let required = match specification.get("required") {
            Some(JsonValue::Bool(required)) => *required,
            _ => {
                return Err(CodegenError::new(format!(
                    "{path}: context value `{id}` has invalid required flag"
                )));
            }
        };
        specifications.insert(id.to_owned(), (value_type.to_owned(), required));
    }
    for (id, value) in &values {
        let (expected_type, _) = specifications
            .get(id)
            .ok_or_else(|| CodegenError::new(format!("{path}: unexpected context value `{id}`")))?;
        if let Some(actual_type) = fixture_canonical_value_type(value, path)? {
            if actual_type != expected_type {
                return Err(CodegenError::new(format!(
                    "{path}: context value `{id}` has type `{actual_type}`, expected `{expected_type}`"
                )));
            }
        }
    }
    for (id, (_, required)) in &specifications {
        if *required
            && values
                .binary_search_by(|candidate| candidate.0.cmp(id))
                .is_err()
        {
            return Err(CodegenError::new(format!(
                "{path}: required context value `{id}` is missing"
            )));
        }
    }

    let supplied = required_string(input, "context_fingerprint", "evaluation fixture input")?;
    let mut digest_input = b"bir-rules/context-value-snapshot/v1\0".to_vec();
    digest_input.extend(runtime_context_snapshot_json(&values)?);
    let computed = sha256_hex(&digest_input);
    if supplied != computed {
        return Err(CodegenError::new(format!(
            "{path}: context_fingerprint `{supplied}` does not match canonical context snapshot `{computed}`"
        )));
    }
    Ok(())
}

fn fixture_canonical_value_type<'a>(value: &'a JsonValue, path: &str) -> Result<Option<&'a str>> {
    let value = required_object(value, "evaluation fixture canonical value")?;
    match required_string(value, "type", "evaluation fixture canonical value")? {
        "absent" | "blank" => Ok(None),
        "text" => Ok(Some("string")),
        "boolean" => Ok(Some("boolean")),
        "integer" => Ok(Some("integer")),
        "decimal" => Ok(Some("decimal")),
        "date" => Ok(Some("date")),
        kind => Err(CodegenError::new(format!(
            "{path}: unsupported canonical context value type `{kind}`"
        ))),
    }
}

fn runtime_context_snapshot_json(values: &[(String, JsonValue)]) -> Result<Vec<u8>> {
    let mut output = String::from("{\"values\":[");
    for (index, (id, value)) in values.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":");
        output.push_str(
            &serde_json::to_string(id).expect("serializing a validated context ID cannot fail"),
        );
        output.push_str(",\"value\":");
        append_runtime_canonical_value_json(&mut output, value)?;
        output.push('}');
    }
    output.push_str("]}");
    Ok(output.into_bytes())
}

fn append_runtime_canonical_value_json(output: &mut String, value: &JsonValue) -> Result<()> {
    let value = required_object(value, "evaluation fixture canonical value")?;
    let kind = required_string(value, "type", "evaluation fixture canonical value")?;
    output.push_str("{\"type\":");
    output.push_str(
        &serde_json::to_string(kind).expect("serializing a canonical value tag cannot fail"),
    );
    match kind {
        "absent" | "blank" => {}
        "text" => {
            let payload = required_string(value, "value", "evaluation fixture canonical value")?;
            output.push_str(",\"value\":");
            output.push_str(
                &serde_json::to_string(payload)
                    .expect("serializing a canonical string value cannot fail"),
            );
        }
        "decimal" => {
            let payload = required_string(value, "value", "evaluation fixture canonical value")?;
            let coefficient = payload.replace('.', "");
            if coefficient.parse::<i128>().is_err() {
                return Err(CodegenError::new(format!(
                    "evaluation fixture canonical decimal `{payload}` exceeds the runtime exact-decimal range"
                )));
            }
            output.push_str(",\"value\":");
            output.push_str(
                &serde_json::to_string(payload)
                    .expect("serializing a canonical decimal value cannot fail"),
            );
        }
        "boolean" => {
            let payload = match value.get("value") {
                Some(JsonValue::Bool(payload)) => *payload,
                _ => {
                    return Err(CodegenError::new(
                        "evaluation fixture canonical boolean has invalid value",
                    ));
                }
            };
            output.push_str(",\"value\":");
            output.push_str(if payload { "true" } else { "false" });
        }
        "integer" => {
            let payload = match value.get("value") {
                Some(JsonValue::Number(payload)) => payload,
                _ => {
                    return Err(CodegenError::new(
                        "evaluation fixture canonical integer has invalid value",
                    ));
                }
            };
            output.push_str(",\"value\":");
            output.push_str(&payload.to_string());
        }
        "date" => {
            let payload = required_object(
                value.get("value").ok_or_else(|| {
                    CodegenError::new("evaluation fixture canonical date is missing value")
                })?,
                "evaluation fixture canonical date",
            )?;
            let year = required_u64(payload, "year", "evaluation fixture canonical date")?;
            let month = required_u64(payload, "month", "evaluation fixture canonical date")?;
            let day = required_u64(payload, "day", "evaluation fixture canonical date")?;
            let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
            let maximum_day = match month {
                2 if leap => 29,
                2 => 28,
                4 | 6 | 9 | 11 => 30,
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                _ => 0,
            };
            if year == 0 || day == 0 || day > maximum_day {
                return Err(CodegenError::new(format!(
                    "evaluation fixture canonical date {year:04}-{month:02}-{day:02} is not a real calendar date"
                )));
            }
            output.push_str(&format!(
                ",\"value\":{{\"year\":{year},\"month\":{month},\"day\":{day}}}"
            ));
        }
        other => {
            return Err(CodegenError::new(format!(
                "unsupported evaluation fixture canonical value `{other}`"
            )));
        }
    }
    output.push('}');
    Ok(())
}

fn fixture_optional_group_instance(
    object: &BTreeMap<String, JsonValue>,
    key: &str,
    label: &str,
) -> Result<Option<FixtureGroupInstance>> {
    match object.get(key) {
        Some(JsonValue::Null) => Ok(None),
        Some(value) => fixture_group_instance(value, label).map(Some),
        None => Err(CodegenError::new(format!(
            "{label} is missing required nullable `{key}`"
        ))),
    }
}

fn execution_instances_for_scope(
    scope: &EvaluationScope,
    group_instances: &[FixtureGroupInstance],
) -> Vec<Option<FixtureGroupInstance>> {
    match scope {
        EvaluationScope::Singleton => vec![None],
        EvaluationScope::EachGroup { group_id } => group_instances
            .iter()
            .filter(|instance| instance.group_id == *group_id)
            .cloned()
            .map(Some)
            .collect(),
    }
}

fn executable_rule_expectations(
    document: &RuleSetDocument,
    profile: &str,
    phase: &str,
    group_instances: &[FixtureGroupInstance],
) -> Result<Vec<FixtureRuleExpectation>> {
    let mut expectations = Vec::new();
    for rule in &document.rules {
        let rule = required_object(rule, "rule")?;
        if profile_branch_is_executable(rule, profile, "rule")?
            && required_string_array(rule, "phases", "rule")?.contains(&phase)
        {
            let scope = parse_evaluation_scope(
                rule.get("scope")
                    .ok_or_else(|| CodegenError::new("rule is missing scope"))?,
                "rule.scope",
            )?;
            let rule_id = required_string(rule, "rule_id", "rule")?.to_owned();
            let order = required_u64(rule, "order", "rule")?;
            expectations.extend(
                execution_instances_for_scope(&scope, group_instances)
                    .into_iter()
                    .map(|instance| FixtureRuleExpectation {
                        execution: FixtureRuleExecution {
                            rule_id: rule_id.clone(),
                            instance,
                        },
                        order,
                    }),
            );
        }
    }
    expectations.sort_by(|left, right| {
        (
            left.order,
            left.execution.rule_id.as_str(),
            left.execution.instance.as_ref(),
        )
            .cmp(&(
                right.order,
                right.execution.rule_id.as_str(),
                right.execution.instance.as_ref(),
            ))
    });
    Ok(expectations)
}

fn all_executable_rule_ids(document: &RuleSetDocument, profile: &str) -> Result<BTreeSet<String>> {
    document
        .rules
        .iter()
        .filter_map(|rule| {
            let rule = match required_object(rule, "rule") {
                Ok(rule) => rule,
                Err(error) => return Some(Err(error)),
            };
            match profile_branch_is_executable(rule, profile, "rule") {
                Ok(true) => Some(required_string(rule, "rule_id", "rule").map(str::to_owned)),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect()
}

fn all_executable_issue_rule_ids(
    document: &RuleSetDocument,
    profile: &str,
) -> Result<BTreeSet<String>> {
    let mut rule_ids = BTreeSet::new();
    for rule in &document.rules {
        let rule = required_object(rule, "rule")?;
        let branch = profile_branch(rule, profile, "rule")?;
        if branch.get("state").and_then(JsonValue::as_str) != Some("executable") {
            continue;
        }
        let emits_issue = required_array(branch, "effects", "executable rule profile")?
            .iter()
            .any(|effect| {
                effect
                    .object()
                    .and_then(|effect| effect.get("kind"))
                    .and_then(JsonValue::as_str)
                    == Some("emit-issue")
            });
        if emits_issue {
            rule_ids.insert(required_string(rule, "rule_id", "rule")?.to_owned());
        }
    }
    Ok(rule_ids)
}

fn executable_output_expectations(
    document: &RuleSetDocument,
    profile: &str,
    phase: &str,
    group_instances: &[FixtureGroupInstance],
) -> Result<Vec<FixtureOutputExpectation>> {
    let calculations = document
        .calculations
        .iter()
        .map(|calculation| {
            let calculation = required_object(calculation, "calculation")?;
            Ok((
                required_string(calculation, "calculation_id", "calculation")?,
                calculation,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut expectations = Vec::new();
    for calculation_id in &document.evaluation_order {
        let calculation = calculations.get(calculation_id.as_str()).ok_or_else(|| {
            CodegenError::new(format!(
                "evaluation_order references missing calculation `{calculation_id}`"
            ))
        })?;
        if profile_branch_is_executable(calculation, profile, "calculation")?
            && required_string_array(calculation, "phases", "calculation")?.contains(&phase)
        {
            let scope = calculation_evaluation_scope(calculation, "calculation")?;
            for instance in execution_instances_for_scope(&scope, group_instances) {
                for output_id in required_string_array(calculation, "output_ids", "calculation")? {
                    expectations.push(FixtureOutputExpectation {
                        calculation_id: calculation_id.clone(),
                        output_id: output_id.to_owned(),
                        instance: instance.clone(),
                    });
                }
            }
        }
    }
    Ok(expectations)
}

fn all_executable_outputs(
    document: &RuleSetDocument,
    profile: &str,
) -> Result<BTreeSet<(String, String)>> {
    let mut outputs = BTreeSet::new();
    for calculation in &document.calculations {
        let calculation = required_object(calculation, "calculation")?;
        if !profile_branch_is_executable(calculation, profile, "calculation")? {
            continue;
        }
        let calculation_id = required_string(calculation, "calculation_id", "calculation")?;
        for output_id in required_string_array(calculation, "output_ids", "calculation")? {
            outputs.insert((calculation_id.to_owned(), output_id.to_owned()));
        }
    }
    Ok(outputs)
}

fn profile_branch_is_executable(
    definition: &BTreeMap<String, JsonValue>,
    profile: &str,
    label: &str,
) -> Result<bool> {
    let branch = profile_branch(definition, profile, label)?;
    Ok(branch.get("state").and_then(JsonValue::as_str) == Some("executable"))
}

fn profile_branch<'a>(
    definition: &'a BTreeMap<String, JsonValue>,
    profile: &str,
    label: &str,
) -> Result<&'a BTreeMap<String, JsonValue>> {
    let profiles = required_object(
        definition
            .get("profiles")
            .ok_or_else(|| CodegenError::new(format!("{label} missing profiles")))?,
        &format!("{label} profiles"),
    )?;
    let branch = required_object(
        profiles
            .get(profile)
            .ok_or_else(|| CodegenError::new(format!("{label} missing `{profile}` profile")))?,
        &format!("{label} `{profile}` profile"),
    )?;
    Ok(branch)
}

fn fixture_rule_expectations(
    path: &str,
    report: &BTreeMap<String, JsonValue>,
) -> Result<Vec<FixtureRuleExpectation>> {
    required_array(report, "expected_rules", "evaluation fixture report")?
        .iter()
        .map(|rule| {
            let rule = required_object(rule, "evaluation fixture expected rule")?;
            let execution = required_object(
                rule.get("execution").ok_or_else(|| {
                    CodegenError::new("evaluation fixture expected rule missing execution")
                })?,
                "evaluation fixture expected rule execution",
            )?;
            Ok(FixtureRuleExpectation {
                execution: FixtureRuleExecution {
                    rule_id: required_string(
                        execution,
                        "rule_id",
                        "evaluation fixture expected rule execution",
                    )?
                    .to_owned(),
                    instance: fixture_optional_group_instance(
                        execution,
                        "instance",
                        "evaluation fixture expected rule execution",
                    )?,
                },
                order: required_u64(rule, "order", "evaluation fixture expected rule")?,
            })
        })
        .collect::<Result<Vec<_>>>()
        .map_err(|error| CodegenError::new(format!("{path}: {error}")))
}

fn validate_evaluated_rule_projection(
    path: &str,
    report: &BTreeMap<String, JsonValue>,
    expected: &[FixtureRuleExpectation],
) -> Result<()> {
    let evaluated = required_array(report, "evaluated_rules", "evaluation fixture report")?
        .iter()
        .map(|execution| {
            let execution =
                required_object(execution, "evaluation fixture evaluated rule execution")?;
            Ok(FixtureRuleExecution {
                rule_id: required_string(
                    execution,
                    "rule_id",
                    "evaluation fixture evaluated rule execution",
                )?
                .to_owned(),
                instance: fixture_optional_group_instance(
                    execution,
                    "instance",
                    "evaluation fixture evaluated rule execution",
                )?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let projected = expected
        .iter()
        .map(|rule| rule.execution.clone())
        .collect::<Vec<_>>();
    if evaluated == projected {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "{path}: evaluated_rules do not exactly project expected_rules"
        )))
    }
}

fn validate_violation_context(
    document: &RuleSetDocument,
    path: &str,
    report: &BTreeMap<String, JsonValue>,
    phase: &str,
    profile: &str,
    expected_rules: &[FixtureRuleExpectation],
) -> Result<BTreeSet<String>> {
    let expected_executions = expected_rules
        .iter()
        .map(|rule| rule.execution.clone())
        .collect::<BTreeSet<_>>();
    let issue_rule_ids = all_executable_issue_rule_ids(document, profile)?;
    let mut violated = BTreeSet::new();
    let mut previous_order = None;
    for violation in required_array(report, "violations", "evaluation fixture report")? {
        let violation = required_object(violation, "evaluation fixture violation")?;
        let execution = required_object(
            violation.get("execution").ok_or_else(|| {
                CodegenError::new(format!("{path}: violation is missing rule execution"))
            })?,
            "evaluation fixture violation execution",
        )?;
        let execution = FixtureRuleExecution {
            rule_id: required_string(
                execution,
                "rule_id",
                "evaluation fixture violation execution",
            )?
            .to_owned(),
            instance: fixture_optional_group_instance(
                execution,
                "instance",
                "evaluation fixture violation execution",
            )?,
        };
        if !expected_executions.contains(&execution) {
            return Err(CodegenError::new(format!(
                "{path}: violation references rule execution `{execution:?}` outside expected_rules"
            )));
        }
        if !issue_rule_ids.contains(execution.rule_id.as_str()) {
            return Err(CodegenError::new(format!(
                "{path}: violation references executable rule `{}` without an emit-issue effect for profile `{profile}`",
                execution.rule_id
            )));
        }
        let expected_order = expected_rules
            .iter()
            .find(|rule| rule.execution == execution)
            .expect("execution membership was checked above")
            .order;
        let order = required_object(
            violation
                .get("order")
                .ok_or_else(|| CodegenError::new(format!("{path}: violation is missing order")))?,
            "evaluation fixture violation order",
        )?;
        let rule_order = required_u64(order, "rule_order", "evaluation fixture violation order")?;
        let occurrence = required_u64(order, "occurrence", "evaluation fixture violation order")?;
        if rule_order != expected_order {
            return Err(CodegenError::new(format!(
                "{path}: violation for `{execution:?}` uses rule_order {rule_order}, expected {expected_order}"
            )));
        }
        let current_order = (rule_order, occurrence);
        if previous_order.is_some_and(|previous| previous >= current_order) {
            return Err(CodegenError::new(format!(
                "{path}: violation orders must be strictly increasing; {current_order:?} follows {previous_order:?}"
            )));
        }
        previous_order = Some(current_order);
        for (key, expected) in [("phase", phase), ("profile", profile)] {
            let actual = required_string(violation, key, "evaluation fixture violation")?;
            if actual != expected {
                return Err(CodegenError::new(format!(
                    "{path}: violation {key} `{actual}` differs from fixture `{expected}`"
                )));
            }
        }
        violated.insert(execution.rule_id);
    }
    Ok(violated)
}

fn fixture_output_expectations(
    path: &str,
    expected: &BTreeMap<String, JsonValue>,
    key: &str,
) -> Result<Vec<FixtureOutputExpectation>> {
    required_array(expected, key, "evaluation fixture expected result")?
        .iter()
        .map(|output| {
            let output = required_object(output, "evaluation fixture output")?;
            Ok(FixtureOutputExpectation {
                calculation_id: required_string(
                    output,
                    "calculation_id",
                    "evaluation fixture output",
                )?
                .to_owned(),
                output_id: required_string(output, "output_id", "evaluation fixture output")?
                    .to_owned(),
                instance: fixture_optional_group_instance(
                    output,
                    "instance",
                    "evaluation fixture output",
                )?,
            })
        })
        .collect::<Result<Vec<_>>>()
        .map_err(|error| CodegenError::new(format!("{path}: {error}")))
}

fn validate_exact_output_coverage(
    path: &str,
    profile: &str,
    phase: &str,
    applicable: &[FixtureOutputExpectation],
    actual: &[FixtureOutputExpectation],
) -> Result<()> {
    if actual == applicable {
        return Ok(());
    }
    let expected_set = applicable.iter().cloned().collect::<BTreeSet<_>>();
    let actual_set = actual.iter().cloned().collect::<BTreeSet<_>>();
    let missing = expected_set
        .difference(&actual_set)
        .cloned()
        .collect::<Vec<_>>();
    let extra = actual_set
        .difference(&expected_set)
        .cloned()
        .collect::<Vec<_>>();
    Err(CodegenError::new(format!(
        "{path}: expected_outputs must exactly match ordered executable `{profile}` outputs for phase `{phase}`; missing {missing:?}, extra {extra:?}, expected order {applicable:?}, found {actual:?}"
    )))
}

fn ordered_set_mismatch<T>(
    label: &str,
    expected: &BTreeSet<T>,
    actual: &BTreeSet<T>,
) -> CodegenError
where
    T: Clone + std::fmt::Debug + Ord,
{
    let missing = expected.difference(actual).cloned().collect::<Vec<_>>();
    let extra = actual.difference(expected).cloned().collect::<Vec<_>>();
    CodegenError::new(format!(
        "{label} mismatch; missing: {missing:?}; extra: {extra:?}"
    ))
}

fn validate_workflow_references(workflow: &JsonValue) -> Result<()> {
    let object = required_object(workflow, "workflow")?;
    if object.get("state").is_some() {
        return Ok(());
    }
    let states = match object.get("states") {
        Some(JsonValue::Array(states)) => states,
        _ => return Err(CodegenError::new("typed workflow is missing states")),
    };
    let state_ids = collect_object_ids(states, "state_id")?;
    let initial = required_string(object, "initial_state", "workflow")?;
    if !state_ids.contains(initial) {
        return Err(CodegenError::new(format!(
            "workflow initial_state `{initial}` does not resolve"
        )));
    }
    let transitions = match object.get("transitions") {
        Some(JsonValue::Array(transitions)) => transitions,
        _ => return Err(CodegenError::new("typed workflow is missing transitions")),
    };
    for transition in transitions {
        let transition = required_object(transition, "workflow transition")?;
        for key in ["from_state", "to_state"] {
            let state = required_string(transition, key, "workflow transition")?;
            if !state_ids.contains(state) {
                return Err(CodegenError::new(format!(
                    "workflow {key} `{state}` does not resolve"
                )));
            }
        }
    }
    validate_string_references(
        workflow,
        "state_id",
        &state_ids,
        "workflow state",
        "$.workflow",
    )
}

fn validate_derived_outputs(document: &RuleSetDocument, rule_set: &JsonValue) -> Result<()> {
    let mut outputs = BTreeMap::<String, BTreeSet<String>>::new();
    for calculation in &document.calculations {
        let object = required_object(calculation, "calculation")?;
        let id = required_string(object, "calculation_id", "calculation")?;
        let declared = required_string_array(object, "output_ids", "calculation")?
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        validate_executable_output_coverage(id, object, &declared)?;
        outputs.insert(id.to_owned(), declared);
    }
    walk_json(rule_set, "$", &mut |value, path| {
        let JsonValue::Object(object) = value else {
            return Ok(());
        };
        if object.get("kind").and_then(JsonValue::as_str) != Some("derived") {
            return Ok(());
        }
        let calculation = required_string(object, "calculation_id", "derived expression")?;
        let output = required_string(object, "output_id", "derived expression")?;
        if !outputs
            .get(calculation)
            .is_some_and(|outputs| outputs.contains(output))
        {
            return Err(CodegenError::new(format!(
                "{path}: derived output `{calculation}.{output}` does not resolve"
            )));
        }
        Ok(())
    })
}

fn validate_executable_output_coverage(
    calculation_id: &str,
    calculation: &BTreeMap<String, JsonValue>,
    declared: &BTreeSet<String>,
) -> Result<()> {
    let profiles = required_object(
        calculation
            .get("profiles")
            .ok_or_else(|| CodegenError::new("calculation missing profiles"))?,
        "calculation profiles",
    )?;
    for profile in ["official", "filing_safe"] {
        let branch = required_object(
            profiles
                .get(profile)
                .ok_or_else(|| CodegenError::new("calculation profile missing"))?,
            "calculation profile",
        )?;
        if branch.get("state").and_then(JsonValue::as_str) != Some("executable") {
            continue;
        }
        let outputs = match branch.get("outputs") {
            Some(JsonValue::Array(outputs)) => collect_object_ids(outputs, "output_id")?
                .into_iter()
                .map(str::to_owned)
                .collect::<BTreeSet<String>>(),
            _ => return Err(CodegenError::new("executable calculation missing outputs")),
        };
        if &outputs != declared {
            return Err(CodegenError::new(format!(
                "calculation `{calculation_id}` {profile} outputs do not match output_ids"
            )));
        }
    }
    Ok(())
}

fn serialization_contract_digest(rule_set: &JsonValue) -> Result<String> {
    rule_set
        .object()
        .and_then(|object| object.get("serialization"))
        .map(canonical_bytes)
        .map(|bytes| sha256_hex(&bytes))
        .ok_or_else(|| {
            CodegenError::new("rule set is missing serialization after schema validation")
        })
}

pub(crate) fn snapshot_source_digest(
    rule_set: &JsonValue,
    fixtures: &BTreeMap<String, JsonValue>,
) -> Result<String> {
    let mut normalized = rule_set.clone();
    let identity = normalized
        .object_mut()
        .and_then(|root| root.get_mut("identity"))
        .and_then(JsonValue::object_mut)
        .ok_or_else(|| CodegenError::new("rule set identity is not an object"))?;
    identity.insert("source_set_sha256".to_owned(), JsonValue::Null);

    let mut entries = BTreeMap::from([("rule-set.json".to_owned(), canonical_bytes(&normalized))]);
    for (path, fixture) in fixtures {
        let mut normalized_fixture = fixture.clone();
        normalize_evaluation_fixture_identity_digests(&mut normalized_fixture)?;
        entries.insert(path.clone(), canonical_bytes(&normalized_fixture));
    }
    Ok(digest_owned_entries(
        "bir-rules-snapshot-source-set-v1",
        &entries,
    ))
}

fn normalize_evaluation_fixture_identity_digests(fixture: &mut JsonValue) -> Result<()> {
    let is_evaluation = fixture
        .object()
        .and_then(|fixture| fixture.get("kind"))
        .and_then(JsonValue::as_str)
        == Some("evaluation");
    if !is_evaluation {
        return Ok(());
    }

    for (path, label) in [
        (
            &["input", "rule_set"][..],
            "evaluation fixture input rule_set",
        ),
        (
            &["expected", "report", "rule_set"][..],
            "evaluation fixture expected report rule_set",
        ),
    ] {
        let rule_set = required_nested_object_mut(fixture, path, label)?;
        if !rule_set.contains_key("source_set_sha256") {
            return Err(CodegenError::new(format!(
                "{label} missing source_set_sha256 after schema validation"
            )));
        }
        rule_set.insert("source_set_sha256".to_owned(), JsonValue::Null);
    }
    if fixture
        .object()
        .is_some_and(|fixture| fixture.contains_key("workflow_transition"))
    {
        let rule_set = required_nested_object_mut(
            fixture,
            &["workflow_transition", "expected", "rule_set"],
            "evaluation fixture expected workflow rule_set",
        )?;
        if !rule_set.contains_key("source_set_sha256") {
            return Err(CodegenError::new(
                "evaluation fixture expected workflow rule_set missing source_set_sha256 after schema validation",
            ));
        }
        rule_set.insert("source_set_sha256".to_owned(), JsonValue::Null);
    }
    Ok(())
}

fn required_nested_object_mut<'a>(
    value: &'a mut JsonValue,
    path: &[&str],
    label: &str,
) -> Result<&'a mut BTreeMap<String, JsonValue>> {
    if path.is_empty() {
        return value.object_mut().ok_or_else(|| {
            CodegenError::new(format!("{label} must be an object after schema validation"))
        });
    }
    let object = value.object_mut().ok_or_else(|| {
        CodegenError::new(format!(
            "{label} parent must be an object after schema validation"
        ))
    })?;
    let next = object.get_mut(path[0]).ok_or_else(|| {
        CodegenError::new(format!(
            "{label} path component `{}` is missing after schema validation",
            path[0]
        ))
    })?;
    required_nested_object_mut(next, &path[1..], label)
}

fn digest_owned_entries(domain: &str, entries: &BTreeMap<String, Vec<u8>>) -> String {
    digest_entries(
        domain,
        entries
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    )
}

fn collect_object_ids<'a>(values: &'a [JsonValue], key: &str) -> Result<BTreeSet<&'a str>> {
    let mut ids = BTreeSet::new();
    for value in values {
        let object = required_object(value, "definition")?;
        let id = required_string(object, key, "definition")?;
        if !ids.insert(id) {
            return Err(CodegenError::new(format!("duplicate {key} `{id}`")));
        }
    }
    Ok(ids)
}

fn validate_string_references(
    value: &JsonValue,
    key: &str,
    definitions: &BTreeSet<&str>,
    label: &str,
    root_path: &str,
) -> Result<()> {
    walk_json(value, root_path, &mut |value, path| {
        let JsonValue::Object(object) = value else {
            return Ok(());
        };
        if let Some(JsonValue::String(reference)) = object.get(key) {
            if !definitions.contains(reference.as_str()) {
                return Err(CodegenError::new(format!(
                    "{path}/{key}: missing {label} reference `{reference}`"
                )));
            }
        }
        Ok(())
    })
}

fn validate_optional_string_references(
    value: &JsonValue,
    key: &str,
    definitions: &BTreeSet<&str>,
    label: &str,
    root_path: &str,
) -> Result<()> {
    validate_string_references(value, key, definitions, label, root_path)
}

fn validate_array_references(
    values: &[JsonValue],
    key: &str,
    definitions: &BTreeSet<&str>,
    label: &str,
    root_path: &str,
) -> Result<()> {
    for (index, value) in values.iter().enumerate() {
        let object = required_object(value, "reference owner")?;
        if let Some(JsonValue::Array(references)) = object.get(key) {
            for reference in references.iter().filter_map(JsonValue::as_str) {
                if !definitions.contains(reference) {
                    return Err(CodegenError::new(format!(
                        "{root_path}/{index}/{key}: missing {label} reference `{reference}`"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn collect_derived_calculation_ids(value: &JsonValue, output: &mut BTreeSet<String>) {
    match value {
        JsonValue::Object(object) => {
            if object.get("kind").and_then(JsonValue::as_str) == Some("derived") {
                if let Some(JsonValue::String(calculation)) = object.get("calculation_id") {
                    output.insert(calculation.clone());
                }
            }
            for value in object.values() {
                collect_derived_calculation_ids(value, output);
            }
        }
        JsonValue::Array(values) => {
            for value in values {
                collect_derived_calculation_ids(value, output);
            }
        }
        _ => {}
    }
}

fn contains_state(value: &JsonValue, state: &str) -> bool {
    match value {
        JsonValue::Object(object) => {
            object.get("state").and_then(JsonValue::as_str) == Some(state)
                || object.values().any(|value| contains_state(value, state))
        }
        JsonValue::Array(values) => values.iter().any(|value| contains_state(value, state)),
        _ => false,
    }
}

fn reject_machine_local_paths(value: &JsonValue, root_path: &str) -> Result<()> {
    walk_json(value, root_path, &mut |value, path| {
        let JsonValue::String(value) = value else {
            return Ok(());
        };
        let bytes = value.as_bytes();
        let drive_absolute = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\');
        let machine_local = drive_absolute
            || value.starts_with(r"\\")
            || value.starts_with("file://")
            || value.starts_with("/Users/")
            || value.starts_with("/home/")
            || value.starts_with("/Volumes/");
        if machine_local {
            return Err(CodegenError::new(format!(
                "{path}: machine-local path `{value}` is forbidden in v2 source"
            )));
        }
        Ok(())
    })
}

fn walk_json(
    value: &JsonValue,
    path: &str,
    visitor: &mut impl FnMut(&JsonValue, &str) -> Result<()>,
) -> Result<()> {
    visitor(value, path)?;
    match value {
        JsonValue::Object(object) => {
            for (key, value) in object {
                walk_json(
                    value,
                    &format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                    visitor,
                )?;
            }
        }
        JsonValue::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                walk_json(value, &format!("{path}/{index}"), visitor)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn required_object<'a>(
    value: &'a JsonValue,
    label: &str,
) -> Result<&'a BTreeMap<String, JsonValue>> {
    value.object().ok_or_else(|| {
        CodegenError::new(format!("{label} must be an object after schema validation"))
    })
}

fn required_string<'a>(
    object: &'a BTreeMap<String, JsonValue>,
    key: &str,
    label: &str,
) -> Result<&'a str> {
    object.get(key).and_then(JsonValue::as_str).ok_or_else(|| {
        CodegenError::new(format!(
            "{label} `{key}` must be a string after schema validation"
        ))
    })
}

fn required_array<'a>(
    object: &'a BTreeMap<String, JsonValue>,
    key: &str,
    label: &str,
) -> Result<&'a [JsonValue]> {
    match object.get(key) {
        Some(JsonValue::Array(values)) => Ok(values),
        _ => Err(CodegenError::new(format!(
            "{label} `{key}` must be an array after schema validation"
        ))),
    }
}

fn required_string_array<'a>(
    object: &'a BTreeMap<String, JsonValue>,
    key: &str,
    label: &str,
) -> Result<Vec<&'a str>> {
    match object.get(key) {
        Some(JsonValue::Array(values)) => values
            .iter()
            .map(|value| {
                value.as_str().ok_or_else(|| {
                    CodegenError::new(format!(
                        "{label} `{key}` item must be a string after schema validation"
                    ))
                })
            })
            .collect(),
        _ => Err(CodegenError::new(format!(
            "{label} `{key}` must be an array after schema validation"
        ))),
    }
}

fn required_u64(object: &BTreeMap<String, JsonValue>, key: &str, label: &str) -> Result<u64> {
    match object.get(key) {
        Some(JsonValue::Number(number)) => number.as_u64().ok_or_else(|| {
            CodegenError::new(format!("{label} `{key}` must be a nonnegative integer"))
        }),
        _ => Err(CodegenError::new(format!(
            "{label} `{key}` must be an integer after schema validation"
        ))),
    }
}

fn array_len_property(value: &JsonValue, key: &str) -> Result<u64> {
    let object = required_object(value, "legacy source")?;
    match object.get(key) {
        Some(JsonValue::Array(values)) => Ok(values.len() as u64),
        _ => Err(CodegenError::new(format!(
            "legacy source `{key}` must be an array"
        ))),
    }
}

fn set_mismatch(
    label: &str,
    expected: &BTreeSet<String>,
    actual: &BTreeSet<String>,
) -> CodegenError {
    let missing = expected.difference(actual).cloned().collect::<Vec<_>>();
    let extra = actual.difference(expected).cloned().collect::<Vec<_>>();
    CodegenError::new(format!(
        "{label} mismatch; missing: {missing:?}; extra: {extra:?}"
    ))
}

#[cfg(test)]
mod evaluation_policy_contract_tests {
    use super::*;
    use serde_json::json;

    fn policy_contract() -> (SchemaSet, JsonValue, PathBuf) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let schema_root = manifest_dir.join("../../rules/schema/v2");
        let rule_set_path =
            manifest_dir.join("../../rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json");
        let schemas = SchemaSet::load(&schema_root).expect("load repository v2 schemas");
        let rule_set = parse_strict(
            &read_bytes(&rule_set_path).expect("read scaffold rule set"),
            &rule_set_path,
        )
        .expect("parse scaffold rule set");
        (schemas, rule_set, rule_set_path)
    }

    #[test]
    fn schema_requires_a_closed_profiled_evaluation_policy() {
        let (schemas, rule_set, _) = policy_contract();
        schemas
            .validate("rule-set.schema.json", &rule_set)
            .expect("honest scaffold policy validates");

        let mut missing = rule_set.clone();
        missing.object_mut().unwrap().remove("evaluation_policy");
        assert!(
            schemas.validate("rule-set.schema.json", &missing).is_err(),
            "evaluation_policy must not default"
        );

        let mut invalid_mode = rule_set;
        invalid_mode
            .object_mut()
            .unwrap()
            .get_mut("evaluation_policy")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "official".to_owned(),
                serde_json::from_value(json!({
                    "state": "executable",
                    "effect_mode": "infer-from-prose",
                    "review_decision": {"source_id": "v1-validations"},
                    "source_refs": [{"source_id": "v1-validations"}]
                }))
                .unwrap(),
            );
        assert!(
            schemas
                .validate("rule-set.schema.json", &invalid_mode)
                .is_err(),
            "effect mode must be a closed reviewed value"
        );
    }

    #[test]
    fn schema_rejects_reviewed_non_executable_evaluation_policy_branches() {
        let (schemas, mut reviewed, _) = policy_contract();
        let root = reviewed.object_mut().unwrap();
        root.insert(
            "review_status".to_owned(),
            JsonValue::String("reviewed".to_owned()),
        );
        root.get_mut("identity")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "source_set_sha256".to_owned(),
                JsonValue::String(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                ),
            );
        root.insert(
            "profile_status".to_owned(),
            serde_json::from_value(json!({
                "official": {
                    "state": "executable",
                    "review_decision": {"source_id": "v1-validations"},
                    "source_refs": [{"source_id": "v1-validations"}]
                },
                "filing_safe": {
                    "state": "executable",
                    "review_decision": {"source_id": "v1-validations"},
                    "source_refs": [{"source_id": "v1-validations"}]
                }
            }))
            .unwrap(),
        );

        assert!(
            schemas.validate("rule-set.schema.json", &reviewed).is_err(),
            "reviewed documented/unresolved policy branches must fail schema validation"
        );

        reviewed.object_mut().unwrap().insert(
            "evaluation_policy".to_owned(),
            serde_json::from_value(json!({
                "official": {
                    "state": "executable",
                    "effect_mode": "stop-effects-after-first-blocking-issue",
                    "review_decision": {"source_id": "v1-validations"},
                    "source_refs": [{"source_id": "v1-validations"}]
                },
                "filing_safe": {
                    "state": "executable",
                    "effect_mode": "apply-all",
                    "review_decision": {"source_id": "v1-validations"},
                    "source_refs": [{"source_id": "v1-validations"}]
                }
            }))
            .unwrap(),
        );
        schemas
            .validate("rule-set.schema.json", &reviewed)
            .expect("reviewed schema accepts two explicit executable policy branches");
    }

    #[test]
    fn evaluation_policy_source_references_must_resolve() {
        let (_, mut rule_set, _) = policy_contract();
        let policy = rule_set
            .object_mut()
            .unwrap()
            .get_mut("evaluation_policy")
            .unwrap()
            .object_mut()
            .unwrap();
        let official = policy.get_mut("official").unwrap().object_mut().unwrap();
        let JsonValue::Array(source_refs) = official.get_mut("source_refs").unwrap() else {
            panic!("official policy source_refs is an array");
        };
        source_refs[0].object_mut().unwrap().insert(
            "source_id".to_owned(),
            JsonValue::String("missing-policy-source".to_owned()),
        );

        let document: RuleSetDocument =
            serde_json::from_value(rule_set.clone().into_serde()).unwrap();
        let error = validate_references(&document, &rule_set, &BTreeMap::new())
            .expect_err("unresolved evaluation-policy source must fail");
        assert!(error.message().contains("missing-policy-source"));
        assert!(error.message().contains("missing source"));
    }
}

#[cfg(test)]
mod serialization_contract_tests {
    use super::*;
    use crate::model::{
        SerializationAbsentPolicy, SerializationBlankPolicy, SerializationDecimalSeparator,
        SerializationGrouping, SerializationNegativeRepresentation, SerializationPresentFormat,
        SerializationRoundingMode, SerializationSemanticFormat,
    };
    use serde_json::json;

    fn occurrence(
        key_language: SerializationKeyLanguage,
        start: u32,
        always_present: bool,
        path: &str,
    ) -> SerializationOccurrenceUse {
        SerializationOccurrenceUse {
            key_label: key_language.identity(),
            key_language,
            start,
            end: Some(start),
            step: 1,
            fixed_cardinality: true,
            always_present,
            path: path.to_owned(),
        }
    }

    fn indexed_key(base: u32, maximum: usize) -> SerializationKeyLanguage {
        SerializationKeyLanguage::Indexed {
            group_id: "rows".to_owned(),
            index_base: base,
            index_step: 1,
            padding: 0,
            prefix: "foo".to_owned(),
            suffix: String::new(),
            max_occurs: Some(maximum),
        }
    }

    fn decimal_format(
        grouping: SerializationGrouping,
        decimal_separator: SerializationDecimalSeparator,
    ) -> SerializationSemanticFormat {
        SerializationSemanticFormat {
            absent: SerializationAbsentPolicy::Reject,
            blank: SerializationBlankPolicy::Reject,
            present: SerializationPresentFormat::Decimal {
                scale: 2,
                rounding: SerializationRoundingMode::HalfEven,
                grouping,
                decimal_separator,
                negative: SerializationNegativeRepresentation::LeadingMinus,
            },
        }
    }

    fn text_format(
        absent: SerializationAbsentPolicy,
        blank: SerializationBlankPolicy,
    ) -> SerializationSemanticFormat {
        SerializationSemanticFormat {
            absent,
            blank,
            present: SerializationPresentFormat::Text,
        }
    }

    #[test]
    fn decimal_formatter_rejects_ambiguous_separator_and_type_mismatch() {
        let ambiguous = decimal_format(
            SerializationGrouping::Comma,
            SerializationDecimalSeparator::Comma,
        );
        let error = validate_serialization_format(&ambiguous, "decimal", "$.format")
            .expect_err("grouping and decimal separator must differ");
        assert!(error.message().contains("must differ"));

        let valid = decimal_format(
            SerializationGrouping::Comma,
            SerializationDecimalSeparator::Period,
        );
        let error = validate_serialization_format(&valid, "integer", "$.format")
            .expect_err("formatter/value mismatch must fail");
        assert!(error.message().contains("formatter/value type mismatch"));
    }

    #[test]
    fn dynamic_group_nesting_is_closed_until_parent_child_identity_exists() {
        validate_dynamic_group_nesting(&[], "$.nodes[0]")
            .expect("a top-level dynamic group remains supported");

        let scopes = [SerializationGroupScope {
            group_id: "parent-rows",
            min_occurs: 0,
            max_occurs: Some(2),
        }];
        let error = validate_dynamic_group_nesting(&scopes, "$.nodes[0].nodes[0]")
            .expect_err("a nested dynamic group must fail closed");
        assert!(
            error
                .message()
                .contains("nested dynamic groups are unsupported")
        );
        assert!(error.message().contains("parent-rows"));
    }

    #[test]
    fn exact_and_indexed_key_languages_cannot_overlap() {
        let occurrences = vec![
            occurrence(
                SerializationKeyLanguage::Exact("foo1".to_owned()),
                1,
                true,
                "$.exact",
            ),
            occurrence(indexed_key(1, 2), 1, true, "$.indexed"),
        ];
        let error = validate_serialization_occurrences(&occurrences, "$.nodes")
            .expect_err("exact/indexed overlap must fail");
        assert!(error.message().contains("same physical key"));
    }

    #[test]
    fn different_indexed_sequences_cannot_overlap() {
        let occurrences = vec![
            occurrence(indexed_key(1, 3), 1, true, "$.base-one"),
            occurrence(indexed_key(2, 3), 1, true, "$.base-two"),
        ];
        let error = validate_serialization_occurrences(&occurrences, "$.nodes")
            .expect_err("indexed languages intersect at foo2");
        assert!(error.message().contains("same physical key"));
    }

    #[test]
    fn conditional_or_omitted_earlier_occurrence_cannot_shift_later_output() {
        let key = SerializationKeyLanguage::Exact("same".to_owned());
        let occurrences = vec![
            occurrence(key.clone(), 1, false, "$.conditional"),
            occurrence(key, 2, true, "$.later"),
        ];
        let error = validate_serialization_occurrences(&occurrences, "$.nodes")
            .expect_err("conditional numbering shift must fail");
        assert!(error.message().contains("physical occurrence"));
    }

    #[test]
    fn semantically_absent_earlier_occurrence_cannot_shift_later_output() {
        let format = text_format(
            SerializationAbsentPolicy::OmitOccurrence,
            SerializationBlankPolicy::Reject,
        );
        let key = SerializationKeyLanguage::Exact("same".to_owned());
        let occurrences = vec![
            occurrence(
                key.clone(),
                1,
                serialization_node_always_emits(&SerializationPresence::Always, &format),
                "$.absent",
            ),
            occurrence(key, 2, true, "$.later"),
        ];
        let error = validate_serialization_occurrences(&occurrences, "$.nodes")
            .expect_err("absent omission must make the earlier occurrence optional");
        assert!(error.message().contains("physical occurrence"));
    }

    #[test]
    fn semantically_blank_earlier_occurrence_cannot_shift_later_output() {
        let format = text_format(
            SerializationAbsentPolicy::Reject,
            SerializationBlankPolicy::OmitOccurrence,
        );
        let key = SerializationKeyLanguage::Exact("same".to_owned());
        let occurrences = vec![
            occurrence(
                key.clone(),
                1,
                serialization_node_always_emits(&SerializationPresence::Always, &format),
                "$.blank",
            ),
            occurrence(key, 2, true, "$.later"),
        ];
        let error = validate_serialization_occurrences(&occurrences, "$.nodes")
            .expect_err("blank omission must make the earlier occurrence optional");
        assert!(error.message().contains("physical occurrence"));
    }

    #[test]
    fn semantic_omission_cannot_govern_a_repeated_projected_range() {
        let format = text_format(
            SerializationAbsentPolicy::OmitOccurrence,
            SerializationBlankPolicy::Reject,
        );
        let mut repeated = occurrence(
            SerializationKeyLanguage::Exact("same".to_owned()),
            1,
            serialization_node_always_emits(&SerializationPresence::Always, &format),
            "$.group-range",
        );
        repeated.end = Some(3);
        repeated.fixed_cardinality = false;
        let error = validate_serialization_occurrences(&[repeated], "$.nodes")
            .expect_err("semantic omission over a repeated range must fail");
        assert!(error.message().contains("repeated occurrence range"));
    }

    #[test]
    fn ordered_compare_rejects_boolean_and_null_but_boolean_equality_is_valid() {
        for operand_type in ["boolean", "null"] {
            let error =
                validate_serialization_compare_operator("less-than", operand_type, "$.compare")
                    .expect_err("ordered comparison must reject unordered types");
            assert!(error.message().contains("invalid"));
        }
        validate_serialization_compare_operator("equal", "boolean", "$.compare")
            .expect("same-type boolean equality remains valid");
    }

    #[test]
    fn group_indexed_projections_require_bounded_groups() {
        let scopes = [SerializationGroupScope {
            group_id: "rows",
            min_occurs: 0,
            max_occurs: None,
        }];
        let key: SerializationKeyProjection = serde_json::from_value(json!({
            "kind": "group-indexed",
            "group_id": "rows",
            "index_base": 1,
            "index_step": 1,
            "padding": 0,
            "prefix": "row",
            "suffix": "",
            "review_decision": {"source_id": "review"},
            "source_refs": [{"source_id": "review"}]
        }))
        .expect("deserialize indexed key");
        let error = validate_key_projection(&key, &scopes, "$.key")
            .expect_err("unbounded indexed key must fail");
        assert!(error.message().contains("requires a bounded dynamic group"));

        let occurrence: SerializationOccurrenceProjection = serde_json::from_value(json!({
            "kind": "group-indexed",
            "group_id": "rows",
            "index_base": 1,
            "index_step": 1,
            "review_decision": {"source_id": "review"},
            "source_refs": [{"source_id": "review"}]
        }))
        .expect("deserialize indexed occurrence");
        let error = validate_occurrence_projection(&occurrence, &scopes, "$.occurrence")
            .expect_err("unbounded indexed occurrence must fail");
        assert!(error.message().contains("requires a bounded dynamic group"));
    }

    #[test]
    fn bounded_projection_rejects_arithmetic_overflow_and_key_endpoint_growth() {
        let error = validate_bounded_index(u32::MAX, 1, Some(2), "$.key", "key")
            .expect_err("bounded endpoint overflow must fail");
        assert!(error.message().contains("exceeds u32"));

        let scopes = [SerializationGroupScope {
            group_id: "rows",
            min_occurs: 2,
            max_occurs: Some(2),
        }];
        let prefix = "a".repeat(254);
        let key: SerializationKeyProjection = serde_json::from_value(json!({
            "kind": "group-indexed",
            "group_id": "rows",
            "index_base": 9,
            "index_step": 1,
            "padding": 0,
            "prefix": prefix,
            "suffix": "",
            "review_decision": {"source_id": "review"},
            "source_refs": [{"source_id": "review"}]
        }))
        .expect("deserialize indexed key");
        let error = validate_key_projection(&key, &scopes, "$.key")
            .expect_err("endpoint length growth beyond the stable-ID bound must fail");
        assert!(error.message().contains("generated key endpoint"));
    }

    #[test]
    fn serialization_digest_is_deterministic_and_changes_with_subtree_source() {
        let first: JsonValue = serde_json::from_value(json!({
            "serialization": {
                "contract_version": "1.0.0",
                "artifacts": []
            }
        }))
        .unwrap();
        let second: JsonValue = serde_json::from_value(json!({
            "serialization": {
                "contract_version": "1.0.0",
                "artifacts": [{
                    "artifact_id": "changed"
                }]
            }
        }))
        .unwrap();
        let first_digest = serialization_contract_digest(&first).unwrap();
        assert_eq!(first_digest, serialization_contract_digest(&first).unwrap());
        assert_ne!(
            first_digest,
            serialization_contract_digest(&second).unwrap()
        );
    }
}

#[cfg(test)]
mod scoped_evaluation_contract_tests {
    use super::*;
    use serde_json::json;

    fn scoped_document() -> RuleSetDocument {
        serde_json::from_value(json!({
            "$schema": "../../../schema/v2/rule-set.schema.json",
            "schema_version": "2.0.0",
            "identity": {
                "rule_set_id": "test-v1-p1",
                "form_code": "TEST",
                "form_revision": "2024-01-01",
                "official_package_version": "1.0.0",
                "source_set_sha256": null
            },
            "review_status": "skeleton",
            "profile_status": {
                "official": {
                    "state": "documented_only",
                    "summary": "test",
                    "source_refs": [{"source_id": "review"}]
                },
                "filing_safe": {
                    "state": "unresolved",
                    "reason": "test",
                    "source_refs": [{"source_id": "review"}]
                }
            },
            "evaluation_policy": {
                "official": {
                    "state": "documented_only",
                    "summary": "test",
                    "source_refs": [{"source_id": "review"}]
                },
                "filing_safe": {
                    "state": "unresolved",
                    "reason": "test",
                    "source_refs": [{"source_id": "review"}]
                }
            },
            "sources": [],
            "legacy_v1": {
                "form_id": "test-v1",
                "schema_version": "1.0.0",
                "mappings": [],
                "declared_counts": {
                    "typed_fields": 0,
                    "concrete_union_fields": 0,
                    "field_groups": 0,
                    "validation_rules": 0,
                    "calculations": 0,
                    "workflow_states": 0,
                    "workflow_transitions": 0,
                    "negative_fixtures": 0,
                    "confirmed_official_bugs": 0,
                    "unverified_gaps": 0
                }
            },
            "context_values": [],
            "field_groups": [{
                "group_id": "rows",
                "min_occurs": 0,
                "max_occurs": 10,
                "members": ["row-amount"]
            }],
            "fields": [{
                "field_id": "row-amount",
                "value_type": "decimal",
                "group_id": "rows"
            }],
            "evaluation_order": ["row-tax", "total"],
            "calculations": [{
                "calculation_id": "row-tax",
                "scope": {"kind": "each-group", "group_id": "rows"},
                "output_ids": ["value"],
                "depends_on": [],
                "phases": ["validate"],
                "profiles": {
                    "official": {
                        "state": "executable",
                        "condition": {"kind": "constant", "value": true},
                        "outputs": [{
                            "output_id": "value",
                            "value": {
                                "kind": "field",
                                "result_type": "decimal",
                                "field": {
                                    "field_id": "row-amount",
                                    "instance": {"kind": "current-group-instance"}
                                }
                            },
                            "rounding": null
                        }],
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    },
                    "filing_safe": {
                        "state": "executable",
                        "condition": {"kind": "constant", "value": true},
                        "outputs": [{
                            "output_id": "value",
                            "value": {
                                "kind": "field",
                                "result_type": "decimal",
                                "field": {
                                    "field_id": "row-amount",
                                    "instance": {"kind": "current-group-instance"}
                                }
                            },
                            "rounding": null
                        }],
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    }
                },
                "source_refs": [{"source_id": "review"}]
            }, {
                "calculation_id": "total",
                "scope": {"kind": "singleton"},
                "output_ids": ["value"],
                "depends_on": ["row-tax"],
                "phases": ["validate"],
                "profiles": {
                    "official": {
                        "state": "executable",
                        "condition": {"kind": "constant", "value": true},
                        "outputs": [{
                            "output_id": "value",
                            "value": {
                                "kind": "group-aggregate",
                                "result_type": "decimal",
                                "operator": "sum",
                                "group_id": "rows",
                                "value": {
                                    "kind": "derived",
                                    "result_type": "decimal",
                                    "calculation_id": "row-tax",
                                    "output_id": "value",
                                    "instance": {"kind": "current-group-instance"}
                                }
                            },
                            "rounding": null
                        }],
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    },
                    "filing_safe": {
                        "state": "executable",
                        "condition": {"kind": "constant", "value": true},
                        "outputs": [{
                            "output_id": "value",
                            "value": {
                                "kind": "group-aggregate",
                                "result_type": "decimal",
                                "operator": "sum",
                                "group_id": "rows",
                                "value": {
                                    "kind": "derived",
                                    "result_type": "decimal",
                                    "calculation_id": "row-tax",
                                    "output_id": "value",
                                    "instance": {"kind": "current-group-instance"}
                                }
                            },
                            "rounding": null
                        }],
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    }
                },
                "source_refs": [{"source_id": "review"}]
            }],
            "rules": [{
                "rule_id": "row-valid",
                "scope": {"kind": "each-group", "group_id": "rows"},
                "order": 1,
                "phases": ["validate"],
                "field_ids": ["row-amount"],
                "profiles": {
                    "official": {
                        "state": "executable",
                        "predicate": {
                            "kind": "is-present",
                            "value": {
                                "kind": "derived",
                                "result_type": "decimal",
                                "calculation_id": "row-tax",
                                "output_id": "value",
                                "instance": {"kind": "current-group-instance"}
                            }
                        },
                        "effects": [],
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    },
                    "filing_safe": {
                        "state": "executable",
                        "predicate": {"kind": "constant", "value": true},
                        "effects": [],
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    }
                },
                "source_refs": [{"source_id": "review"}]
            }],
            "workflow": {"state": "unresolved", "reason": "test", "source_refs": [{"source_id": "review"}]},
            "serialization": {"contract_version": "1.0.0", "artifacts": []},
            "fixtures": []
        }))
        .expect("scoped test document deserializes")
    }

    fn calculation_output_value_mut<'a>(
        document: &'a mut RuleSetDocument,
        calculation_index: usize,
    ) -> &'a mut JsonValue {
        let calculation = document.calculations[calculation_index]
            .object_mut()
            .unwrap();
        let profiles = calculation
            .get_mut("profiles")
            .unwrap()
            .object_mut()
            .unwrap();
        let branch = profiles.get_mut("official").unwrap().object_mut().unwrap();
        let JsonValue::Array(outputs) = branch.get_mut("outputs").unwrap() else {
            panic!("outputs are an array");
        };
        outputs[0].object_mut().unwrap().get_mut("value").unwrap()
    }

    fn scoped_fixture_input(instance_ids: &[&str]) -> JsonValue {
        let instances = instance_ids
            .iter()
            .map(|instance_id| {
                json!({
                    "group_id": "rows",
                    "instance_id": instance_id
                })
            })
            .collect::<Vec<_>>();
        let fields = instance_ids
            .iter()
            .map(|instance_id| {
                json!({
                    "field": {
                        "field_id": "row-amount",
                        "group_path": [{
                            "group_id": "rows",
                            "instance_id": instance_id
                        }]
                    },
                    "value": {"state": "text", "text": "1.00"}
                })
            })
            .collect::<Vec<_>>();
        let mut fingerprint_input = b"bir-rules/context-value-snapshot/v1\0".to_vec();
        fingerprint_input.extend_from_slice(br#"{"values":[]}"#);
        let context_fingerprint = sha256_hex(&fingerprint_input);
        serde_json::from_value(json!({
            "context_fingerprint": context_fingerprint,
            "context_values": {"values": []},
            "raw_inputs": {
                "repeated_group_instances": instances,
                "fields": fields
            }
        }))
        .expect("scoped fixture input deserializes")
    }

    #[test]
    fn scoped_cross_references_and_expression_aggregate_are_accepted() {
        let document = scoped_document();
        validate_scoped_evaluation_contract(&document).expect("scoped references are compatible");
        validate_calculation_graph(&document).expect("scoped dependencies are ordered and exact");
    }

    #[test]
    fn mismatched_derived_selector_and_nested_aggregate_fail_closed() {
        let mut selector_mismatch = scoped_document();
        let rule = selector_mismatch.rules[0].object_mut().unwrap();
        let profiles = rule.get_mut("profiles").unwrap().object_mut().unwrap();
        let branch = profiles.get_mut("official").unwrap().object_mut().unwrap();
        let predicate = branch.get_mut("predicate").unwrap().object_mut().unwrap();
        let derived = predicate.get_mut("value").unwrap().object_mut().unwrap();
        derived.insert(
            "instance".to_owned(),
            serde_json::from_value(json!({"kind": "singleton"})).unwrap(),
        );
        let error = validate_scoped_evaluation_contract(&selector_mismatch)
            .expect_err("group-derived output cannot use singleton selector");
        assert!(error.message().contains("incompatible"));

        let mut nested = scoped_document();
        let inner = calculation_output_value_mut(&mut nested, 1).clone();
        *calculation_output_value_mut(&mut nested, 1) = serde_json::from_value(json!({
            "kind": "group-aggregate",
            "result_type": "decimal",
            "operator": "sum",
            "group_id": "rows",
            "value": inner
        }))
        .unwrap();
        let error = validate_scoped_evaluation_contract(&nested)
            .expect_err("nested aggregate must fail closed");
        assert!(error.message().contains("nested group aggregate"));
    }

    #[test]
    fn aggregate_and_derived_declared_types_fail_before_emission() {
        let mut count_type = scoped_document();
        calculation_output_value_mut(&mut count_type, 1)
            .object_mut()
            .unwrap()
            .insert("operator".to_owned(), JsonValue::String("count".to_owned()));
        let error = validate_scoped_evaluation_contract(&count_type)
            .expect_err("count aggregates must declare an integer result");
        assert!(error.message().contains("must be `integer`"));

        let mut invalid_sum = scoped_document();
        let aggregate = calculation_output_value_mut(&mut invalid_sum, 1)
            .object_mut()
            .unwrap();
        aggregate.insert(
            "result_type".to_owned(),
            JsonValue::String("string".to_owned()),
        );
        aggregate.insert(
            "value".to_owned(),
            serde_json::from_value(json!({
                "kind": "literal",
                "value": {"type": "string", "value": "not numeric"}
            }))
            .unwrap(),
        );
        let error = validate_scoped_evaluation_contract(&invalid_sum)
            .expect_err("sum aggregates must reject non-numeric values");
        assert!(error.message().contains("invalid for value type `string`"));

        let mut derived_type = scoped_document();
        let rule = derived_type.rules[0].object_mut().unwrap();
        let profiles = rule.get_mut("profiles").unwrap().object_mut().unwrap();
        let branch = profiles.get_mut("official").unwrap().object_mut().unwrap();
        let predicate = branch.get_mut("predicate").unwrap().object_mut().unwrap();
        predicate
            .get_mut("value")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "result_type".to_owned(),
                JsonValue::String("integer".to_owned()),
            );
        let error = validate_scoped_evaluation_contract(&derived_type)
            .expect_err("derived declared type must match the referenced output");
        assert!(
            error
                .message()
                .contains("differs from `row-tax.value` type")
        );
    }

    #[test]
    fn dependency_must_be_available_for_every_owner_phase_and_profile() {
        let mut document = scoped_document();
        document.calculations[0].object_mut().unwrap().insert(
            "phases".to_owned(),
            serde_json::from_value(json!(["save"])).unwrap(),
        );
        let error =
            validate_calculation_graph(&document).expect_err("dependency phase mismatch must fail");
        assert!(error.message().contains("unavailable in phase"));
    }

    #[test]
    fn fixture_inventory_expands_exact_group_instances() {
        let document = scoped_document();
        let instances = vec![
            FixtureGroupInstance {
                group_id: "rows".to_owned(),
                instance_id: "row-1".to_owned(),
            },
            FixtureGroupInstance {
                group_id: "rows".to_owned(),
                instance_id: "row-2".to_owned(),
            },
        ];
        let rules =
            executable_rule_expectations(&document, "official", "validate", &instances).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].execution.instance, Some(instances[0].clone()));
        assert_eq!(rules[1].execution.instance, Some(instances[1].clone()));

        let outputs =
            executable_output_expectations(&document, "official", "validate", &instances).unwrap();
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].instance, Some(instances[0].clone()));
        assert_eq!(outputs[1].instance, Some(instances[1].clone()));
        assert_eq!(outputs[2].instance, None);
    }

    #[test]
    fn fixture_request_shape_rejects_missing_row_fields_and_excess_rows() {
        let document = scoped_document();
        let mut missing = scoped_fixture_input(&["row-1"]);
        let raw_inputs = missing
            .object_mut()
            .unwrap()
            .get_mut("raw_inputs")
            .unwrap()
            .object_mut()
            .unwrap();
        raw_inputs.insert("fields".to_owned(), JsonValue::Array(Vec::new()));
        let error = validate_fixture_input(
            &document,
            "missing-row-field.json",
            missing.object().unwrap(),
        )
        .expect_err("a declared row without every row field must fail");
        assert!(
            error
                .message()
                .contains("raw fixture field coverage mismatch")
        );

        let instance_ids = (0..11)
            .map(|index| format!("row-{index:02}"))
            .collect::<Vec<_>>();
        let instance_refs = instance_ids.iter().map(String::as_str).collect::<Vec<_>>();
        let excessive = scoped_fixture_input(&instance_refs);
        let error =
            validate_fixture_input(&document, "too-many-rows.json", excessive.object().unwrap())
                .expect_err("fixture rows beyond max_occurs must fail");
        assert!(error.message().contains("fixture cardinality 11"));
    }

    #[test]
    fn fixture_canonical_inputs_must_exactly_echo_and_type_every_raw_field() {
        let document = scoped_document();
        let input = scoped_fixture_input(&["row-1"]);
        let validated =
            validate_fixture_input(&document, "canonical.json", input.object().unwrap())
                .expect("valid fixture input");
        let expected: JsonValue = serde_json::from_value(json!({
            "canonical_inputs": [{
                "field": {
                    "field_id": "row-amount",
                    "group_path": [{
                        "group_id": "rows",
                        "instance_id": "row-1"
                    }]
                },
                "raw": {"state": "text", "text": "1.00"},
                "canonical": {"type": "decimal", "value": "1"}
            }]
        }))
        .unwrap();
        validate_fixture_canonical_inputs(
            &document,
            "canonical.json",
            expected.object().unwrap(),
            &validated.raw_fields,
        )
        .expect("canonical input exactly covers and echoes the raw field");

        let two_rows = scoped_fixture_input(&["row-1", "row-2"]);
        let validated_two_rows =
            validate_fixture_input(&document, "reversed.json", two_rows.object().unwrap())
                .expect("two-row fixture input");
        let reversed: JsonValue = serde_json::from_value(json!({
            "canonical_inputs": [{
                "field": {
                    "field_id": "row-amount",
                    "group_path": [{
                        "group_id": "rows",
                        "instance_id": "row-2"
                    }]
                },
                "raw": {"state": "text", "text": "1.00"},
                "canonical": {"type": "decimal", "value": "1"}
            }, {
                "field": {
                    "field_id": "row-amount",
                    "group_path": [{
                        "group_id": "rows",
                        "instance_id": "row-1"
                    }]
                },
                "raw": {"state": "text", "text": "1.00"},
                "canonical": {"type": "decimal", "value": "1"}
            }]
        }))
        .unwrap();
        let error = validate_fixture_canonical_inputs(
            &document,
            "reversed.json",
            reversed.object().unwrap(),
            &validated_two_rows.raw_fields,
        )
        .expect_err("canonical input order must match runtime FieldInstance ordering");
        assert!(
            error
                .message()
                .contains("must exactly cover raw_inputs in stable field order")
        );

        let mut missing = expected.clone();
        missing
            .object_mut()
            .unwrap()
            .insert("canonical_inputs".to_owned(), JsonValue::Array(Vec::new()));
        let error = validate_fixture_canonical_inputs(
            &document,
            "missing-canonical.json",
            missing.object().unwrap(),
            &validated.raw_fields,
        )
        .expect_err("canonical input coverage must be exact");
        assert!(error.message().contains("must exactly cover raw_inputs"));

        let mut wrong_raw = expected.clone();
        let canonical_inputs = wrong_raw
            .object_mut()
            .unwrap()
            .get_mut("canonical_inputs")
            .unwrap();
        let JsonValue::Array(canonical_inputs) = canonical_inputs else {
            panic!("canonical_inputs is an array");
        };
        canonical_inputs[0]
            .object_mut()
            .unwrap()
            .insert("raw".to_owned(), JsonValue::String("wrong".to_owned()));
        let error = validate_fixture_canonical_inputs(
            &document,
            "wrong-raw.json",
            wrong_raw.object().unwrap(),
            &validated.raw_fields,
        )
        .expect_err("canonical input must echo the exact raw wire value");
        assert!(error.message().contains("does not echo raw_inputs"));

        let mut wrong_type = expected;
        let canonical_inputs = wrong_type
            .object_mut()
            .unwrap()
            .get_mut("canonical_inputs")
            .unwrap();
        let JsonValue::Array(canonical_inputs) = canonical_inputs else {
            panic!("canonical_inputs is an array");
        };
        canonical_inputs[0].object_mut().unwrap().insert(
            "canonical".to_owned(),
            serde_json::from_value(json!({"type": "text", "value": "1"})).unwrap(),
        );
        let error = validate_fixture_canonical_inputs(
            &document,
            "wrong-type.json",
            wrong_type.object().unwrap(),
            &validated.raw_fields,
        )
        .expect_err("canonical type must match the field declaration");
        assert!(
            error
                .message()
                .contains("has type `string`, expected `decimal`")
        );
    }

    #[test]
    fn fixture_inventory_uses_runtime_stable_order_and_requires_every_output_instance() {
        let document = scoped_document();
        let shuffled = scoped_fixture_input(&["row-2", "row-1"]);
        let validated =
            validate_fixture_input(&document, "shuffled.json", shuffled.object().unwrap())
                .expect("runtime request canonicalization accepts shuffled source rows");
        let instances = validated.group_instances;
        assert_eq!(instances[0].instance_id, "row-1");
        assert_eq!(instances[1].instance_id, "row-2");

        let applicable =
            executable_output_expectations(&document, "official", "validate", &instances)
                .expect("build exact output inventory");
        assert_eq!(applicable.len(), 3);
        assert_eq!(
            applicable[0].instance.as_ref().unwrap().instance_id,
            "row-1"
        );
        assert_eq!(
            applicable[1].instance.as_ref().unwrap().instance_id,
            "row-2"
        );

        let mut missing_row = applicable.clone();
        missing_row.remove(1);
        let error = validate_exact_output_coverage(
            "missing-row-output.json",
            "official",
            "validate",
            &applicable,
            &missing_row,
        )
        .expect_err("one row cannot stand in for another row's output");
        assert!(error.message().contains("row-2"));
        assert!(error.message().contains("must exactly match"));
    }
}

#[cfg(test)]
mod reviewed_completeness_tests {
    use super::*;
    use serde_json::json;

    const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn reviewed_document() -> RuleSetDocument {
        serde_json::from_value(json!({
            "$schema": "../../../schema/v2/rule-set.schema.json",
            "schema_version": "2.0.0",
            "identity": {
                "rule_set_id": "test-v1-p1",
                "form_code": "TEST",
                "form_revision": "2024-01-01",
                "official_package_version": "1.0.0",
                "source_set_sha256": DIGEST_A
            },
            "review_status": "reviewed",
            "profile_status": {
                "official": {
                    "state": "executable",
                    "review_decision": {"source_id": "review"},
                    "source_refs": [{"source_id": "review"}]
                },
                "filing_safe": {
                    "state": "executable",
                    "review_decision": {"source_id": "review"},
                    "source_refs": [{"source_id": "review"}]
                }
            },
            "evaluation_policy": {
                "official": {
                    "state": "executable",
                    "effect_mode": "stop-effects-after-first-blocking-issue",
                    "review_decision": {"source_id": "review"},
                    "source_refs": [{"source_id": "review"}]
                },
                "filing_safe": {
                    "state": "executable",
                    "effect_mode": "apply-all",
                    "review_decision": {"source_id": "review"},
                    "source_refs": [{"source_id": "review"}]
                }
            },
            "sources": [],
            "legacy_v1": {
                "form_id": "test-v1",
                "schema_version": "1.0.0",
                "mappings": [
                    {
                        "artifact": "manifest",
                        "source_id": "legacy-manifest",
                        "record_count": 1,
                        "target_sections": ["identity", "sources"],
                        "state": "executable"
                    },
                    {
                        "artifact": "fields",
                        "source_id": "legacy-fields",
                        "record_count": 1,
                        "target_sections": ["field-groups", "fields"],
                        "state": "executable"
                    },
                    {
                        "artifact": "validations",
                        "source_id": "legacy-validations",
                        "record_count": 1,
                        "target_sections": ["rules"],
                        "state": "executable"
                    },
                    {
                        "artifact": "calculations",
                        "source_id": "legacy-calculations",
                        "record_count": 1,
                        "target_sections": ["calculations"],
                        "state": "executable"
                    },
                    {
                        "artifact": "workflow",
                        "source_id": "legacy-workflow",
                        "record_count": 1,
                        "target_sections": ["workflow"],
                        "state": "executable"
                    }
                ],
                "declared_counts": {
                    "typed_fields": 1,
                    "concrete_union_fields": 1,
                    "field_groups": 1,
                    "validation_rules": 1,
                    "calculations": 1,
                    "workflow_states": 1,
                    "workflow_transitions": 1,
                    "negative_fixtures": 2,
                    "confirmed_official_bugs": 0,
                    "unverified_gaps": 0
                }
            },
            "context_values": [],
            "field_groups": [{
                "group_id": "group",
                "min_occurs": 0,
                "max_occurs": null,
                "members": ["field"]
            }],
            "fields": [{
                "field_id": "field",
                "value_type": "string",
                "group_id": "group",
                "serialized": [{
                    "serialized_key": "Field",
                    "occurrence": 1,
                    "document": "editable-save",
                    "presence": "always"
                }],
                "source_refs": [{
                    "source_id": "legacy-fields",
                    "locator": "#/fields/0"
                }]
            }],
            "evaluation_order": ["calculation"],
            "calculations": [{
                "calculation_id": "calculation",
                "scope": {"kind": "singleton"},
                "output_ids": ["output"],
                "depends_on": [],
                "phases": ["validate"],
                "profiles": {
                    "official": {
                        "state": "executable",
                        "outputs": [{"output_id": "output"}]
                    },
                    "filing_safe": {
                        "state": "executable",
                        "outputs": [{"output_id": "output"}]
                    }
                },
                "source_refs": [{
                    "source_id": "legacy-calculations",
                    "locator": "#/calculations/0"
                }]
            }],
            "rules": [{
                "rule_id": "rule",
                "scope": {"kind": "singleton"},
                "order": 10,
                "phases": ["validate"],
                "profiles": {
                    "official": {
                        "state": "executable",
                        "effects": [{"kind": "emit-issue"}]
                    },
                    "filing_safe": {
                        "state": "executable",
                        "effects": [{"kind": "emit-issue"}]
                    }
                },
                "source_refs": [{
                    "source_id": "legacy-validations",
                    "locator": "#/rules/0"
                }]
            }],
            "workflow": {
                "initial_state": "draft",
                "states": [{
                    "state_id": "draft",
                    "terminal": false,
                    "source_refs": [{
                        "source_id": "legacy-workflow",
                        "locator": "#/phases/0"
                    }]
                }],
                "transitions": [{
                    "transition_id": "save",
                    "from_state": "draft",
                    "action": "validate",
                    "evaluation_phase": "validate",
                    "to_state": "draft",
                    "profiles": {
                        "official": {
                            "state": "executable",
                            "guard": {"kind": "constant", "value": true},
                            "effects": [{
                                "kind": "set-workflow-state",
                                "state_id": "draft"
                            }],
                            "review_decision": {"source_id": "review"},
                            "source_refs": [{"source_id": "review"}]
                        },
                        "filing_safe": {
                            "state": "executable",
                            "guard": {"kind": "constant", "value": true},
                            "effects": [{
                                "kind": "set-workflow-state",
                                "state_id": "draft"
                            }],
                            "review_decision": {"source_id": "review"},
                            "source_refs": [{"source_id": "review"}]
                        }
                    },
                    "source_refs": [{
                        "source_id": "legacy-workflow",
                        "locator": "#/transitions/0"
                    }]
                }]
            },
            "serialization": {
                "contract_version": "1.0.0",
                "artifacts": []
            },
            "fixtures": []
        }))
        .expect("reviewed test document deserializes")
    }

    #[test]
    fn sources_reject_duplicate_physical_evidence_aliases() {
        let root =
            std::env::temp_dir().join(format!("bir-rules-source-alias-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("create source alias test directory");
        let root = root
            .canonicalize()
            .expect("canonicalize source alias test directory");
        let bytes = b"reviewed evidence";
        std::fs::write(root.join("evidence.bin"), bytes).expect("write source evidence");
        let digest = sha256_hex(bytes);
        let mut document = reviewed_document();
        document.sources = vec![
            serde_json::from_value(json!({
                "source_id": "first-evidence",
                "kind": "review-decision",
                "path": "evidence.bin",
                "sha256": digest.clone()
            }))
            .expect("deserialize first source"),
            serde_json::from_value(json!({
                "source_id": "aliased-evidence",
                "kind": "review-decision",
                "path": "evidence.bin",
                "sha256": digest
            }))
            .expect("deserialize aliased source"),
        ];

        let error =
            validate_sources(&document, &root).expect_err("physical evidence alias must fail");
        assert!(
            error.message().contains("aliases physical evidence"),
            "unexpected error: {error}"
        );
        assert!(
            error.message().contains("first-evidence"),
            "unexpected error: {error}"
        );
        assert!(
            error.message().contains("aliased-evidence"),
            "unexpected error: {error}"
        );
        std::fs::remove_dir_all(&root).expect("remove source alias test directory");
    }

    fn reviewed_document_with_unbounded_groups(
        group_count: usize,
        members_per_group: usize,
    ) -> RuleSetDocument {
        let mut document = reviewed_document();
        document.field_groups.clear();
        document.fields.clear();

        for group_index in 0..group_count {
            let group_id = format!("group-{group_index}");
            let mut members = Vec::new();
            for member_index in 0..members_per_group {
                let field_index = group_index * members_per_group + member_index;
                let field_id = format!("field-{field_index}");
                members.push(field_id.clone());
                document.fields.push(
                    serde_json::from_value(json!({
                        "field_id": field_id,
                        "group_id": group_id,
                        "serialized": [{
                            "serialized_key": format!("Field{field_index}"),
                            "occurrence": 1,
                            "document": "editable-save",
                            "presence": "always"
                        }],
                        "source_refs": [{
                            "source_id": "legacy-fields",
                            "locator": format!("#/fields/{field_index}")
                        }]
                    }))
                    .expect("grouped test field deserializes"),
                );
            }
            document.field_groups.push(
                serde_json::from_value(json!({
                    "group_id": group_id,
                    "min_occurs": 0,
                    "max_occurs": null,
                    "members": members
                }))
                .expect("grouped test field group deserializes"),
            );
        }

        let member_count = group_count * members_per_group;
        document.legacy_v1.declared_counts.typed_fields = member_count as u64;
        document.legacy_v1.declared_counts.concrete_union_fields = member_count as u64;
        document.legacy_v1.declared_counts.unbounded_family_members = member_count as u64;
        document
    }

    fn index() -> IndexSnapshot {
        serde_json::from_value(json!({
            "rule_set_id": "test-v1-p1",
            "form_code": "TEST",
            "form_revision": "2024-01-01",
            "official_package_version": "1.0.0",
            "source_set_sha256": DIGEST_A,
            "path": "test-v1-p1/rule-set.json",
            "review_status": "reviewed",
            "profile_states": {
                "official": "executable",
                "filing_safe": "executable"
            }
        }))
        .expect("reviewed test index deserializes")
    }

    fn evaluation_fixture(
        profile: &str,
        digest: &str,
        fixture_suffix: &str,
        include_rule: bool,
        include_output: bool,
        violate_rule: bool,
    ) -> JsonValue {
        let expected_rules = if include_rule {
            json!([{
                "execution": {"rule_id": "rule", "instance": null},
                "order": 10
            }])
        } else {
            json!([])
        };
        let evaluated_rules = if include_rule {
            json!([{"rule_id": "rule", "instance": null}])
        } else {
            json!([])
        };
        let expected_outputs = if include_output {
            json!([{
                "calculation_id": "calculation",
                "output_id": "output",
                "instance": null
            }])
        } else {
            json!([])
        };
        let derived_outputs = if include_output {
            json!([{
                "calculation_id": "calculation",
                "output_id": "output",
                "instance": null,
                "value": {"type": "integer", "value": 1}
            }])
        } else {
            json!([])
        };
        let violations = if violate_rule {
            json!([{
                "execution": {"rule_id": "rule", "instance": null},
                "phase": "validate",
                "order": {"rule_order": 10, "occurrence": 0},
                "fields": [],
                "official_message": "Invalid test value",
                "message": "Invalid test value",
                "assessment": "verified-correct",
                "severity": "blocking",
                "profile": profile
            }])
        } else {
            json!([])
        };
        let mut fingerprint_input = b"bir-rules/context-value-snapshot/v1\0".to_vec();
        fingerprint_input.extend_from_slice(br#"{"values":[]}"#);
        let context_fingerprint = sha256_hex(&fingerprint_input);
        serde_json::from_value(json!({
            "$schema": "../../../../schema/v2/fixture.schema.json",
            "schema_version": "2.0.0",
            "fixture_id": format!("fixture-{profile}-{fixture_suffix}"),
            "kind": "evaluation",
            "description": format!("{profile} {fixture_suffix} evaluation"),
            "source_refs": [{"source_id": "review"}],
            "input": {
                "rule_set": {
                    "rule_set_id": "test-v1-p1",
                    "form_code": "TEST",
                    "form_revision": "2024-01-01",
                    "official_package_version": "1.0.0",
                    "source_set_sha256": digest
                },
                "context": {
                    "phase": "validate",
                    "profile": profile
                },
                "input_revision": 1,
                "context_fingerprint": context_fingerprint.clone(),
                "context_values": {"values": []},
                "raw_inputs": {
                    "repeated_group_instances": [],
                    "fields": []
                }
            },
            "expected": {
                "report": {
                    "rule_set": {
                        "rule_set_id": "test-v1-p1",
                        "form_code": "TEST",
                        "form_revision": "2024-01-01",
                        "official_package_version": "1.0.0",
                        "source_set_sha256": digest
                    },
                    "context": {
                        "phase": "validate",
                        "profile": profile
                    },
                    "input_revision": 1,
                    "context_fingerprint": context_fingerprint,
                    "expected_rules": expected_rules,
                    "evaluated_rules": evaluated_rules,
                    "violations": violations
                },
                "canonical_inputs": [],
                "expected_outputs": expected_outputs,
                "derived_outputs": derived_outputs
            }
        }))
        .expect("evaluation fixture deserializes")
    }

    fn profile_fixtures(
        profile: &str,
        digest: &str,
        include_rule: bool,
        include_output: bool,
        include_positive: bool,
        negative_count: usize,
    ) -> BTreeMap<String, JsonValue> {
        let mut fixtures = BTreeMap::new();
        if include_positive {
            fixtures.insert(
                format!("{profile}-positive.json"),
                evaluation_fixture(
                    profile,
                    digest,
                    "positive",
                    include_rule,
                    include_output,
                    false,
                ),
            );
        }
        for index in 0..negative_count {
            fixtures.insert(
                format!("{profile}-negative-{index}.json"),
                evaluation_fixture(
                    profile,
                    digest,
                    &format!("negative-{index}"),
                    include_rule,
                    include_output,
                    true,
                ),
            );
        }
        fixtures
    }

    fn complete_fixtures(digest: &str) -> BTreeMap<String, JsonValue> {
        let mut fixtures = profile_fixtures("official", digest, true, true, true, 2);
        fixtures.extend(profile_fixtures("filing_safe", digest, true, true, true, 2));
        for path in ["official-positive.json", "filing_safe-positive.json"] {
            let fixture = fixtures
                .get_mut(path)
                .expect("complete fixture profile has a positive case");
            let input = fixture
                .object()
                .expect("evaluation fixture is an object")
                .get("input")
                .expect("evaluation fixture has input")
                .clone();
            let workflow_transition = serde_json::from_value(json!({
                "current_state": "draft",
                "action": "validate",
                "expected": {
                        "rule_set": input.object().unwrap().get("rule_set").unwrap(),
                        "context": input.object().unwrap().get("context").unwrap(),
                        "input_revision": input.object().unwrap().get("input_revision").unwrap(),
                    "context_fingerprint": input.object().unwrap().get("context_fingerprint").unwrap(),
                    "transition_id": "save",
                    "from_state": "draft",
                    "action": "validate",
                    "to_state": "draft",
                    "notifications": []
                }
            }))
            .expect("workflow transition fixture deserializes");
            fixture
                .object_mut()
                .expect("evaluation fixture is an object")
                .insert("workflow_transition".to_owned(), workflow_transition);
        }
        fixtures
    }

    fn add_fixture_source_ref(fixture: &mut JsonValue, source_id: &str, locator: &str) {
        let source_ref = serde_json::from_value(json!({
            "source_id": source_id,
            "locator": locator
        }))
        .unwrap();
        let source_refs = fixture
            .object_mut()
            .unwrap()
            .get_mut("source_refs")
            .unwrap();
        let JsonValue::Array(source_refs) = source_refs else {
            panic!("fixture source_refs is an array");
        };
        source_refs.push(source_ref);
    }

    fn complete_legacy_fixture_evidence() -> (
        BTreeMap<String, JsonValue>,
        ReviewedFixtureEvidence,
        ReviewedFixtureEvidence,
    ) {
        let mut fixtures = complete_fixtures(DIGEST_A);
        add_fixture_source_ref(
            fixtures.get_mut("official-negative-0.json").unwrap(),
            "legacy-negative-fixtures",
            "#/cases/0",
        );
        add_fixture_source_ref(
            fixtures.get_mut("official-negative-1.json").unwrap(),
            "legacy-negative-fixtures",
            "#/cases/1",
        );
        add_fixture_source_ref(
            fixtures.get_mut("official-positive.json").unwrap(),
            "legacy-calculation-fixtures",
            "#/cases/0",
        );
        let negative = ReviewedFixtureEvidence {
            source_id: "legacy-negative-fixtures".to_owned(),
            document: serde_json::from_value(json!({
                "schema_version": "1.0.0",
                "form_id": "test-v1",
                "synthetic_only": true,
                "cases": [
                    {
                        "case_id": "negative-0",
                        "phase": "validate",
                        "mutations": {"field": "first"},
                        "expected_message": "Invalid test value",
                        "rule_id": "rule"
                    },
                    {
                        "case_id": "negative-1",
                        "phase": "validate",
                        "mutations": {"field": "second"},
                        "expected_message": "Invalid test value",
                        "rule_id": "rule"
                    }
                ]
            }))
            .unwrap(),
        };
        let calculations = ReviewedFixtureEvidence {
            source_id: "legacy-calculation-fixtures".to_owned(),
            document: serde_json::from_value(json!({
                "schema_version": "1.0.0",
                "form_id": "test-v1",
                "cases": [{
                    "case_id": "calculation-0",
                    "calculation_id": "calculation",
                    "inputs": {"boundary": "synthetic"},
                    "official_output": "synthetic"
                }]
            }))
            .unwrap(),
        };
        (fixtures, negative, calculations)
    }

    fn legacy_artifacts() -> BTreeMap<LegacyArtifact, JsonValue> {
        BTreeMap::from([
            (
                LegacyArtifact::Manifest,
                serde_json::from_value(json!({
                    "schema_version": "1.0.0",
                    "form_id": "test-v1",
                    "form_code": "TEST",
                    "revision": "2024-01-01",
                    "package_version": "1.0.0",
                    "status": "complete",
                    "counts": {
                        "unbounded_families": 1,
                        "typed_fields": 1,
                        "concrete_union_fields": 1,
                        "validation_rules": 1,
                        "calculations": 1,
                        "negative_fixtures": 2,
                        "confirmed_official_bugs": 0,
                        "unverified_gaps": 0
                    }
                }))
                .unwrap(),
            ),
            (
                LegacyArtifact::Fields,
                serde_json::from_value(json!({"fields": [{}]})).unwrap(),
            ),
            (
                LegacyArtifact::Validations,
                serde_json::from_value(json!({"rules": [{}]})).unwrap(),
            ),
            (
                LegacyArtifact::Calculations,
                serde_json::from_value(json!({"calculations": [{}]})).unwrap(),
            ),
            (
                LegacyArtifact::Workflow,
                serde_json::from_value(json!({
                    "phases": [{}],
                    "transitions": [{}]
                }))
                .unwrap(),
            ),
        ])
    }

    fn validate_test_reviewed(
        document: &RuleSetDocument,
        fixtures: &BTreeMap<String, JsonValue>,
    ) -> Result<()> {
        validate_reviewed_typed_counts(document)?;
        validate_reviewed_legacy_mappings(document)?;
        validate_reviewed_completeness_with_artifacts(document, fixtures, &legacy_artifacts())
    }

    #[test]
    fn reviewed_snapshot_accepts_exact_counts_and_complete_profile_coverage() {
        let document = reviewed_document();
        let fixtures = complete_fixtures(DIGEST_A);
        validate_test_reviewed(&document, &fixtures).unwrap();
        validate_fixture_identity(&index(), &document, &fixtures).unwrap();
    }

    #[test]
    fn reviewed_workflow_fixture_must_match_declared_evaluation_phase() {
        let mut document = reviewed_document();
        let workflow = document.workflow.object_mut().unwrap();
        let JsonValue::Array(transitions) = workflow.get_mut("transitions").unwrap() else {
            panic!("reviewed workflow transitions are an array");
        };
        transitions[0].object_mut().unwrap().insert(
            "evaluation_phase".to_owned(),
            JsonValue::String("draft-preview".to_owned()),
        );

        let error = validate_test_reviewed(&document, &complete_fixtures(DIGEST_A))
            .expect_err("workflow fixture phase must remain transition-bound");
        assert!(
            error
                .message()
                .contains("requires evaluated phase `draft-preview`, found `validate`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn reviewed_snapshot_requires_both_evaluation_policy_profiles_executable() {
        let mut document = reviewed_document();
        document.evaluation_policy.filing_safe = crate::model::EvaluationPolicyBranch::Unresolved {
            reason: "no reviewed filing-safe effect policy".to_owned(),
            source_refs: vec![crate::model::SourceRef {
                source_id: "review".to_owned(),
                locator: None,
            }],
        };

        let error = validate_test_reviewed(&document, &complete_fixtures(DIGEST_A))
            .expect_err("reviewed policy must not borrow the official profile");
        assert!(
            error
                .message()
                .contains("both evaluation-policy profiles executable")
        );
        assert!(error.message().contains("filing_safe is Unresolved"));
    }

    #[test]
    fn reviewed_snapshot_rejects_typed_and_concrete_count_mismatches() {
        let fixtures = complete_fixtures(DIGEST_A);
        let mut missing_field = reviewed_document();
        missing_field.fields.clear();
        let error = validate_test_reviewed(&missing_field, &fixtures)
            .expect_err("missing typed field must fail");
        assert!(error.message().contains("typed fields count 0"));

        let mut missing_concrete_mapping = reviewed_document();
        missing_concrete_mapping
            .fields
            .first_mut()
            .unwrap()
            .object_mut()
            .unwrap()
            .insert("serialized".to_owned(), JsonValue::Array(Vec::new()));
        let error = validate_test_reviewed(&missing_concrete_mapping, &fixtures)
            .expect_err("missing concrete serialized field must fail");
        assert!(
            error
                .message()
                .contains("concrete serialized fields count 0")
        );
    }

    #[test]
    fn reviewed_snapshot_binds_every_declared_typed_section_count() {
        fn assert_count_error(document: RuleSetDocument, expected: &str) {
            let error = validate_test_reviewed(&document, &complete_fixtures(DIGEST_A))
                .expect_err("typed section count mismatch must fail");
            assert!(
                error.message().contains(expected),
                "unexpected error: {error}"
            );
        }

        let mut field_groups = reviewed_document();
        field_groups
            .legacy_v1
            .declared_counts
            .unbounded_family_members = 2;
        assert_count_error(field_groups, "unbounded-family member fields count 1");

        let mut rules = reviewed_document();
        rules.legacy_v1.declared_counts.validation_rules = 2;
        assert_count_error(rules, "validation rules count 1");

        let mut calculations = reviewed_document();
        calculations.legacy_v1.declared_counts.calculations = 2;
        assert_count_error(calculations, "calculations count 1");

        let mut workflow_states = reviewed_document();
        workflow_states.legacy_v1.declared_counts.workflow_states = 2;
        assert_count_error(workflow_states, "workflow states count 1");

        let mut workflow_transitions = reviewed_document();
        workflow_transitions
            .legacy_v1
            .declared_counts
            .workflow_transitions = 2;
        assert_count_error(workflow_transitions, "workflow transitions count 1");
    }

    #[test]
    fn reviewed_snapshot_counts_unbounded_family_members_not_logical_groups() {
        let document = reviewed_document_with_unbounded_groups(7, 4);

        assert_eq!(document.field_groups.len(), 7);
        assert_eq!(
            validated_unbounded_family_member_count(&document).unwrap(),
            28
        );
        validate_reviewed_typed_counts(&document)
            .expect("seven logical groups bind all 28 legacy family descriptors");
    }

    #[test]
    fn reviewed_snapshot_rejects_dummy_unbounded_family_members() {
        let mut document = reviewed_document_with_unbounded_groups(7, 4);
        let members = document.field_groups[0]
            .object_mut()
            .unwrap()
            .get_mut("members")
            .unwrap();
        let JsonValue::Array(members) = members else {
            panic!("field group members are an array");
        };
        members[0] = JsonValue::String("dummy-field".to_owned());

        let error = validate_reviewed_typed_counts(&document)
            .expect_err("a dummy member must not satisfy the legacy descriptor count");
        assert!(
            error
                .message()
                .contains("contains missing field `dummy-field`")
        );
    }

    #[test]
    fn reviewed_snapshot_rejects_duplicate_unbounded_family_members() {
        let mut document = reviewed_document_with_unbounded_groups(7, 4);
        let duplicate = match document.field_groups[0]
            .object()
            .unwrap()
            .get("members")
            .unwrap()
        {
            JsonValue::Array(members) => members[0].clone(),
            _ => panic!("field group members are an array"),
        };
        let second_group_members = document.field_groups[1]
            .object_mut()
            .unwrap()
            .get_mut("members")
            .unwrap();
        let JsonValue::Array(second_group_members) = second_group_members else {
            panic!("field group members are an array");
        };
        second_group_members[0] = duplicate;

        let error = validate_reviewed_typed_counts(&document)
            .expect_err("a duplicate member must not satisfy the legacy descriptor count");
        assert!(error.message().contains("belongs to both groups"));
    }

    #[test]
    fn reviewed_snapshot_rejects_unbound_unbounded_family_members() {
        let mut document = reviewed_document_with_unbounded_groups(7, 4);
        document.fields[0]
            .object_mut()
            .unwrap()
            .insert("group_id".to_owned(), JsonValue::Null);

        let error = validate_reviewed_typed_counts(&document)
            .expect_err("an unbound member must not satisfy the legacy descriptor count");
        assert!(
            error
                .message()
                .contains("group_id and field-group membership disagree")
        );
    }

    #[test]
    fn reviewed_snapshot_does_not_count_bounded_group_members_as_legacy_families() {
        let mut document = reviewed_document_with_unbounded_groups(1, 4);
        document.field_groups[0].object_mut().unwrap().insert(
            "max_occurs".to_owned(),
            serde_json::from_value(json!(4)).unwrap(),
        );

        let error = validate_reviewed_typed_counts(&document)
            .expect_err("bounded group members are not legacy unbounded-family descriptors");
        assert!(
            error
                .message()
                .contains("unbounded-family member fields count 0")
        );
    }

    #[test]
    fn reviewed_snapshot_rejects_non_executable_legacy_mapping() {
        let mut document = reviewed_document();
        document.legacy_v1.mappings[1].state = BranchState::DocumentedOnly;
        let error = validate_test_reviewed(&document, &complete_fixtures(DIGEST_A))
            .expect_err("documented-only mapping must fail");
        assert!(error.message().contains("legacy fields mapping"));
        assert!(error.message().contains("must be executable"));
    }

    #[test]
    fn reviewed_snapshot_binds_legacy_manifest_and_rejects_unverified_gaps() {
        let document = reviewed_document();
        let mut artifacts = legacy_artifacts();
        artifacts
            .get_mut(&LegacyArtifact::Manifest)
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "form_code".to_owned(),
                JsonValue::String("OTHER".to_owned()),
            );
        let error = validate_reviewed_completeness_with_artifacts(
            &document,
            &complete_fixtures(DIGEST_A),
            &artifacts,
        )
        .expect_err("legacy manifest identity drift must fail");
        assert!(error.message().contains("manifest form_code"));

        let mut incomplete = legacy_artifacts();
        incomplete
            .get_mut(&LegacyArtifact::Manifest)
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "status".to_owned(),
                JsonValue::String("researching".to_owned()),
            );
        let error = validate_reviewed_completeness_with_artifacts(
            &document,
            &complete_fixtures(DIGEST_A),
            &incomplete,
        )
        .expect_err("incomplete legacy research must not become reviewed executable behavior");
        assert!(error.message().contains("status must be `complete`"));

        let mut count_drift = legacy_artifacts();
        count_drift
            .get_mut(&LegacyArtifact::Manifest)
            .unwrap()
            .object_mut()
            .unwrap()
            .get_mut("counts")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "confirmed_official_bugs".to_owned(),
                serde_json::from_value(json!(1)).unwrap(),
            );
        let error = validate_reviewed_completeness_with_artifacts(
            &document,
            &complete_fixtures(DIGEST_A),
            &count_drift,
        )
        .expect_err("legacy manifest count drift must fail");
        assert!(error.message().contains("confirmed_official_bugs"));

        let mut family_count_drift = legacy_artifacts();
        family_count_drift
            .get_mut(&LegacyArtifact::Manifest)
            .unwrap()
            .object_mut()
            .unwrap()
            .get_mut("counts")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "unbounded_families".to_owned(),
                serde_json::from_value(json!(2)).unwrap(),
            );
        let error = validate_reviewed_completeness_with_artifacts(
            &document,
            &complete_fixtures(DIGEST_A),
            &family_count_drift,
        )
        .expect_err("legacy family count drift must fail");
        assert!(error.message().contains("unbounded_families"));

        let mut gaps = reviewed_document();
        gaps.legacy_v1.declared_counts.unverified_gaps = 1;
        let error = validate_test_reviewed(&gaps, &complete_fixtures(DIGEST_A))
            .expect_err("reviewed snapshots must reject declared gaps");
        assert!(error.message().contains("zero unverified legacy gaps"));
    }

    #[test]
    fn reviewed_fixtures_biject_legacy_negative_cases_and_cover_calculation_cases() {
        let document = reviewed_document();
        let (fixtures, negative, calculations) = complete_legacy_fixture_evidence();
        validate_reviewed_legacy_negative_fixture_bijection(&document, &fixtures, &negative)
            .unwrap();
        validate_reviewed_legacy_calculation_fixture_coverage(&document, &fixtures, &calculations)
            .unwrap();

        let mut changed_message = negative.clone();
        let cases = changed_message
            .document
            .object_mut()
            .unwrap()
            .get_mut("cases")
            .unwrap();
        let JsonValue::Array(cases) = cases else {
            panic!("negative evidence cases is an array");
        };
        cases[0].object_mut().unwrap().insert(
            "expected_message".to_owned(),
            JsonValue::String("different official message".to_owned()),
        );
        let error = validate_reviewed_legacy_negative_fixture_bijection(
            &document,
            &fixtures,
            &changed_message,
        )
        .expect_err("official fixture message drift must fail");
        assert!(error.message().contains("exact official message"));

        let mut missing_negative = fixtures.clone();
        let fixture = missing_negative
            .get_mut("official-negative-1.json")
            .unwrap()
            .object_mut()
            .unwrap();
        let JsonValue::Array(source_refs) = fixture.get_mut("source_refs").unwrap() else {
            panic!("fixture source_refs is an array");
        };
        source_refs.retain(|source_ref| {
            source_ref
                .object()
                .and_then(|source_ref| source_ref.get("source_id"))
                .and_then(JsonValue::as_str)
                != Some("legacy-negative-fixtures")
        });
        let error = validate_reviewed_legacy_negative_fixture_bijection(
            &document,
            &missing_negative,
            &negative,
        )
        .expect_err("every legacy negative case must have one concrete official fixture");
        assert!(
            error
                .message()
                .contains("negative-fixture locator coverage")
        );

        let mut missing_calculation = fixtures;
        let fixture = missing_calculation
            .get_mut("official-positive.json")
            .unwrap()
            .object_mut()
            .unwrap();
        let JsonValue::Array(source_refs) = fixture.get_mut("source_refs").unwrap() else {
            panic!("fixture source_refs is an array");
        };
        source_refs.retain(|source_ref| {
            source_ref
                .object()
                .and_then(|source_ref| source_ref.get("source_id"))
                .and_then(JsonValue::as_str)
                != Some("legacy-calculation-fixtures")
        });
        let error = validate_reviewed_legacy_calculation_fixture_coverage(
            &document,
            &missing_calculation,
            &calculations,
        )
        .expect_err("every legacy calculation case must have concrete output coverage");
        assert!(
            error
                .message()
                .contains("calculation-fixture locator coverage")
        );
    }

    #[test]
    fn reviewed_fixtures_require_both_profiles_and_every_rule_and_output() {
        let document = reviewed_document();

        let official_only = profile_fixtures("official", DIGEST_A, true, true, true, 2);
        let error = validate_reviewed_fixture_coverage(&document, &official_only)
            .expect_err("missing filing-safe fixture must fail");
        assert!(error.message().contains("profile `filing_safe`"));

        let mut missing_rule = complete_fixtures(DIGEST_A);
        missing_rule.retain(|path, _| !path.starts_with("official-"));
        missing_rule.extend(profile_fixtures("official", DIGEST_A, false, true, true, 2));
        let error = validate_reviewed_fixture_coverage(&document, &missing_rule)
            .expect_err("missing expected rule must fail");
        assert!(error.message().contains("expected_rules"));

        let mut missing_output = complete_fixtures(DIGEST_A);
        missing_output.retain(|path, _| !path.starts_with("official-"));
        missing_output.extend(profile_fixtures("official", DIGEST_A, true, false, true, 2));
        let error = validate_reviewed_fixture_coverage(&document, &missing_output)
            .expect_err("uncovered executable output must fail");
        assert!(
            error
                .message()
                .contains("expected_outputs must exactly match")
        );

        let mut no_positive = complete_fixtures(DIGEST_A);
        no_positive.retain(|path, _| path != "official-positive.json");
        let error = validate_reviewed_fixture_coverage(&document, &no_positive)
            .expect_err("missing positive fixture must fail");
        assert!(error.message().contains("zero-violation positive"));

        let mut too_few_negative = complete_fixtures(DIGEST_A);
        too_few_negative.retain(|path, _| path != "official-negative-1.json");
        let error = validate_reviewed_fixture_coverage(&document, &too_few_negative)
            .expect_err("insufficient negative fixtures must fail");
        assert!(
            error
                .message()
                .contains("legacy declared official-package count 2")
        );

        let mut no_filing_safe_negative = complete_fixtures(DIGEST_A);
        no_filing_safe_negative.retain(|path, _| !path.starts_with("filing_safe-negative-"));
        let error = validate_reviewed_fixture_coverage(&document, &no_filing_safe_negative)
            .expect_err("filing-safe needs independent negative evidence");
        assert!(
            error
                .message()
                .contains("`filing_safe` profile must include at least one negative")
        );
    }

    #[test]
    fn reviewed_positive_fixture_must_exercise_executable_behavior() {
        let document = reviewed_document();
        let mut fixtures = complete_fixtures(DIGEST_A);
        let fixture = fixtures.get_mut("official-positive.json").unwrap();
        let fixture = fixture.object_mut().unwrap();
        {
            let input = fixture.get_mut("input").unwrap().object_mut().unwrap();
            input
                .get_mut("context")
                .unwrap()
                .object_mut()
                .unwrap()
                .insert("phase".to_owned(), JsonValue::String("input".to_owned()));
        }
        {
            let expected = fixture.get_mut("expected").unwrap().object_mut().unwrap();
            let report = expected.get_mut("report").unwrap().object_mut().unwrap();
            report
                .get_mut("context")
                .unwrap()
                .object_mut()
                .unwrap()
                .insert("phase".to_owned(), JsonValue::String("input".to_owned()));
            report.insert("expected_rules".to_owned(), JsonValue::Array(Vec::new()));
            report.insert("evaluated_rules".to_owned(), JsonValue::Array(Vec::new()));
            expected.insert("expected_outputs".to_owned(), JsonValue::Array(Vec::new()));
            expected.insert("derived_outputs".to_owned(), JsonValue::Array(Vec::new()));
        }

        let error = validate_reviewed_fixture_coverage(&document, &fixtures)
            .expect_err("a no-op zero-violation fixture is not positive evidence");
        assert!(
            error
                .message()
                .contains("exercising an executable rule or output")
        );
    }

    #[test]
    fn reviewed_fixtures_cover_every_issue_emitting_rule_with_a_violation() {
        let mut document = reviewed_document();
        document.rules.push(
            serde_json::from_value(json!({
                "rule_id": "second-rule",
                "scope": {"kind": "singleton"},
                "order": 20,
                "phases": ["validate"],
                "profiles": {
                    "official": {
                        "state": "executable",
                        "effects": [{"kind": "emit-issue"}]
                    },
                    "filing_safe": {
                        "state": "executable",
                        "effects": [{"kind": "emit-issue"}]
                    }
                },
                "source_refs": [{
                    "source_id": "legacy-validations",
                    "locator": "#/rules/1"
                }]
            }))
            .unwrap(),
        );
        let mut fixtures = complete_fixtures(DIGEST_A);
        for fixture in fixtures.values_mut() {
            let report = fixture
                .object_mut()
                .unwrap()
                .get_mut("expected")
                .unwrap()
                .object_mut()
                .unwrap()
                .get_mut("report")
                .unwrap()
                .object_mut()
                .unwrap();
            match report.get_mut("expected_rules").unwrap() {
                JsonValue::Array(rules) => rules.push(
                    serde_json::from_value(json!({
                        "execution": {
                            "rule_id": "second-rule",
                            "instance": null
                        },
                        "order": 20
                    }))
                    .unwrap(),
                ),
                _ => unreachable!(),
            }
            match report.get_mut("evaluated_rules").unwrap() {
                JsonValue::Array(rules) => rules.push(
                    serde_json::from_value(json!({
                        "rule_id": "second-rule",
                        "instance": null
                    }))
                    .unwrap(),
                ),
                _ => unreachable!(),
            }
        }

        let error = validate_reviewed_fixture_coverage(&document, &fixtures)
            .expect_err("issue-emitting rule without a violation fixture must fail");
        assert!(
            error
                .message()
                .contains("issue-rule violation fixture coverage")
        );
        assert!(error.message().contains("second-rule"));
    }

    #[test]
    fn reviewed_legacy_locators_are_canonical_and_bijective() {
        let mut document = reviewed_document();
        let field = document.fields[0].object_mut().unwrap();
        let source_ref = match field.get_mut("source_refs").unwrap() {
            JsonValue::Array(source_refs) => source_refs[0].object_mut().unwrap(),
            _ => unreachable!(),
        };
        source_ref.insert(
            "locator".to_owned(),
            JsonValue::String("#/fields/01".to_owned()),
        );
        let error = validate_reviewed_legacy_locator_bijections(&document, &legacy_artifacts())
            .expect_err("non-canonical legacy pointer must fail");
        assert!(error.message().contains("non-canonical array index"));

        let mut wrong_source = reviewed_document();
        let field = wrong_source.fields[0].object_mut().unwrap();
        let source_ref = match field.get_mut("source_refs").unwrap() {
            JsonValue::Array(source_refs) => source_refs[0].object_mut().unwrap(),
            _ => unreachable!(),
        };
        source_ref.insert(
            "source_id".to_owned(),
            JsonValue::String("some-other-source".to_owned()),
        );
        let error = validate_reviewed_legacy_locator_bijections(&wrong_source, &legacy_artifacts())
            .expect_err("missing mapped source citation must fail");
        assert!(error.message().contains("must cite exactly one"));

        let mut missing_phase = reviewed_document();
        let workflow = missing_phase.workflow.object_mut().unwrap();
        let state = match workflow.get_mut("states").unwrap() {
            JsonValue::Array(states) => states[0].object_mut().unwrap(),
            _ => unreachable!(),
        };
        let source_ref = match state.get_mut("source_refs").unwrap() {
            JsonValue::Array(source_refs) => source_refs[0].object_mut().unwrap(),
            _ => unreachable!(),
        };
        source_ref.insert(
            "locator".to_owned(),
            JsonValue::String("#/phases/1".to_owned()),
        );
        let error =
            validate_reviewed_legacy_locator_bijections(&missing_phase, &legacy_artifacts())
                .expect_err("workflow state locator must resolve to one legacy phase");
        assert!(error.message().contains("does not resolve"));
    }

    #[test]
    fn legacy_record_cannot_be_both_represented_and_non_runtime() {
        let mut document = reviewed_document();
        document.legacy_v1.record_classifications.push(
            crate::model::LegacyRecordClassification::NonRuntime {
                artifact: LegacyArtifact::Fields,
                legacy_id: Some("legacy-field".to_owned()),
                locator: "#/fields/0".to_owned(),
                reason: crate::model::LegacyNonRuntimeReason::ProvenUnreachable,
                source_refs: vec![crate::model::SourceRef {
                    source_id: "legacy-fields".to_owned(),
                    locator: Some("#/fields/0".to_owned()),
                }],
            },
        );
        let error = validate_legacy_record_classifications(&document, &legacy_artifacts())
            .expect_err("represented record classification must fail");
        assert!(error.message().contains("both represented by v2"));
    }

    #[test]
    fn idless_workflow_records_classify_by_locator_only() {
        let mut document = reviewed_document();
        let workflow = document.workflow.object_mut().unwrap();
        for array_key in ["states", "transitions"] {
            let JsonValue::Array(entities) = workflow.get_mut(array_key).unwrap() else {
                panic!("workflow entities are an array");
            };
            entities[0]
                .object_mut()
                .unwrap()
                .insert("source_refs".to_owned(), JsonValue::Array(Vec::new()));
        }
        document.legacy_v1.record_classifications = vec![
            workflow_classification("#/phases/0"),
            workflow_classification("#/transitions/0"),
        ];
        validate_legacy_record_classifications(&document, &legacy_artifacts())
            .expect("ID-less phases and transitions classify by exact locator");

        let duplicate = document.legacy_v1.record_classifications[0].clone();
        document.legacy_v1.record_classifications.push(duplicate);
        let error = validate_legacy_record_classifications(&document, &legacy_artifacts())
            .expect_err("duplicate workflow classification must fail");
        assert!(error.message().contains("classified more than once"));

        document.legacy_v1.record_classifications.pop();
        let crate::model::LegacyRecordClassification::NonRuntime { legacy_id, .. } =
            &mut document.legacy_v1.record_classifications[0]
        else {
            unreachable!()
        };
        *legacy_id = Some("invented-phase-id".to_owned());
        let error = validate_legacy_record_classifications(&document, &legacy_artifacts())
            .expect_err("workflow classification cannot invent an ID");
        assert!(error.message().contains("must use locator identity"));
    }

    #[test]
    fn id_bearing_classification_still_requires_exact_legacy_id() {
        let mut document = reviewed_document();
        document.rules[0]
            .object_mut()
            .unwrap()
            .insert("source_refs".to_owned(), JsonValue::Array(Vec::new()));
        document.legacy_v1.record_classifications =
            vec![crate::model::LegacyRecordClassification::NonRuntime {
                artifact: LegacyArtifact::Validations,
                legacy_id: None,
                locator: "#/rules/0".to_owned(),
                reason: crate::model::LegacyNonRuntimeReason::ProvenUnreachable,
                source_refs: vec![crate::model::SourceRef {
                    source_id: "legacy-validations".to_owned(),
                    locator: Some("#/rules/0".to_owned()),
                }],
            }];
        let mut artifacts = legacy_artifacts();
        artifacts.insert(
            LegacyArtifact::Validations,
            serde_json::from_value(json!({
                "rules": [{"rule_id": "legacy-rule"}]
            }))
            .unwrap(),
        );
        let error = validate_legacy_record_classifications(&document, &artifacts)
            .expect_err("validation classification without legacy ID must fail");
        assert!(error.message().contains("must name its source `rule_id`"));
    }

    #[test]
    fn represented_workflow_locators_are_exact_unique_and_in_range() {
        let source_ids = BTreeMap::from([
            (LegacyArtifact::Fields, "legacy-fields"),
            (LegacyArtifact::Validations, "legacy-validations"),
            (LegacyArtifact::Calculations, "legacy-calculations"),
            (LegacyArtifact::Workflow, "legacy-workflow"),
        ]);
        let document = reviewed_document();
        let represented = represented_legacy_locators(&document, &source_ids, &legacy_artifacts())
            .expect("workflow locators reconcile");
        assert!(represented.contains(&(LegacyArtifact::Workflow, "#/phases/0".to_owned())));
        assert!(represented.contains(&(LegacyArtifact::Workflow, "#/transitions/0".to_owned())));

        let mut duplicate = reviewed_document();
        let workflow = duplicate.workflow.object_mut().unwrap();
        let JsonValue::Array(states) = workflow.get_mut("states").unwrap() else {
            panic!("workflow states are an array");
        };
        let source_ref = states[0]
            .object()
            .unwrap()
            .get("source_refs")
            .unwrap()
            .clone();
        states.push(
            serde_json::from_value(json!({
                "source_refs": source_ref
            }))
            .unwrap(),
        );
        let error = represented_legacy_locators(&duplicate, &source_ids, &legacy_artifacts())
            .expect_err("duplicate phase locator must fail");
        assert!(error.message().contains("referenced more than once"));

        let mut wrong_array = reviewed_document();
        set_workflow_state_locator(&mut wrong_array, "#/transitions/0");
        let error = represented_legacy_locators(&wrong_array, &source_ids, &legacy_artifacts())
            .expect_err("state cannot cite a transition record");
        assert!(error.message().contains("invalid for the artifact"));

        let mut out_of_range = reviewed_document();
        set_workflow_state_locator(&mut out_of_range, "#/phases/1");
        let error = represented_legacy_locators(&out_of_range, &source_ids, &legacy_artifacts())
            .expect_err("out-of-range phase locator must fail");
        assert!(error.message().contains("out of range"));
    }

    #[test]
    fn supporting_cross_artifact_citations_are_not_reconciliation_authority() {
        let source_ids = BTreeMap::from([
            (LegacyArtifact::Fields, "legacy-fields"),
            (LegacyArtifact::Validations, "legacy-validations"),
            (LegacyArtifact::Calculations, "legacy-calculations"),
            (LegacyArtifact::Workflow, "legacy-workflow"),
        ]);
        let mut document = reviewed_document();
        let calculation = document.calculations[0]
            .object_mut()
            .expect("calculation is an object");
        let JsonValue::Array(source_refs) = calculation
            .get_mut("source_refs")
            .expect("calculation source refs")
        else {
            panic!("calculation source_refs are an array");
        };
        source_refs.push(
            serde_json::from_value(json!({
                "source_id": "legacy-validations",
                "locator": "#/rules/0"
            }))
            .expect("supporting validation source ref"),
        );

        let represented = represented_legacy_locators(&document, &source_ids, &legacy_artifacts())
            .expect("supporting cross-artifact evidence is permitted");
        assert!(
            represented.contains(&(LegacyArtifact::Calculations, "#/calculations/0".to_owned()))
        );
        assert_eq!(
            represented
                .iter()
                .filter(|(artifact, _)| *artifact == LegacyArtifact::Validations)
                .count(),
            1,
            "supporting validation citation must not become a second represented validation"
        );
    }

    fn workflow_classification(locator: &str) -> crate::model::LegacyRecordClassification {
        crate::model::LegacyRecordClassification::NonRuntime {
            artifact: LegacyArtifact::Workflow,
            legacy_id: None,
            locator: locator.to_owned(),
            reason: crate::model::LegacyNonRuntimeReason::NonValidationUiBehavior,
            source_refs: vec![crate::model::SourceRef {
                source_id: "legacy-workflow".to_owned(),
                locator: Some(locator.to_owned()),
            }],
        }
    }

    fn set_workflow_state_locator(document: &mut RuleSetDocument, locator: &str) {
        let workflow = document.workflow.object_mut().unwrap();
        let JsonValue::Array(states) = workflow.get_mut("states").unwrap() else {
            panic!("workflow states are an array");
        };
        let JsonValue::Array(source_refs) = states[0]
            .object_mut()
            .unwrap()
            .get_mut("source_refs")
            .unwrap()
        else {
            panic!("workflow state source_refs are an array");
        };
        source_refs[0]
            .object_mut()
            .unwrap()
            .insert("locator".to_owned(), JsonValue::String(locator.to_owned()));
    }

    #[test]
    fn represented_legacy_locator_must_be_canonical_and_in_range() {
        fn set_rule_locator(document: &mut RuleSetDocument, locator: &str) {
            let source_ref = document.rules[0]
                .object_mut()
                .unwrap()
                .get_mut("source_refs")
                .unwrap();
            let JsonValue::Array(source_refs) = source_ref else {
                panic!("rule source_refs are an array");
            };
            source_refs[0]
                .object_mut()
                .unwrap()
                .insert("locator".to_owned(), JsonValue::String(locator.to_owned()));
        }

        let source_ids = BTreeMap::from([
            (LegacyArtifact::Fields, "legacy-fields"),
            (LegacyArtifact::Validations, "legacy-validations"),
            (LegacyArtifact::Calculations, "legacy-calculations"),
            (LegacyArtifact::Workflow, "legacy-workflow"),
        ]);
        let mut document = reviewed_document();
        set_rule_locator(&mut document, "#/rules/00");
        let error = represented_legacy_locators(&document, &source_ids, &legacy_artifacts())
            .expect_err("non-canonical legacy locator must fail");
        assert!(
            error
                .message()
                .contains("not a canonical JSON-array locator")
        );

        set_rule_locator(&mut document, "#/rules/1");
        let error = represented_legacy_locators(&document, &source_ids, &legacy_artifacts())
            .expect_err("out-of-range legacy locator must fail");
        assert!(error.message().contains("out of range"));
    }

    #[test]
    fn snapshot_digest_normalizes_only_embedded_rule_set_pins() {
        let rule_set = serde_json::from_value(json!({
            "identity": {"source_set_sha256": DIGEST_A},
            "content": "same"
        }))
        .unwrap();
        let first = snapshot_source_digest(&rule_set, &complete_fixtures(DIGEST_A)).unwrap();
        let second = snapshot_source_digest(&rule_set, &complete_fixtures(DIGEST_B)).unwrap();
        assert_eq!(first, second, "embedded self-pins are normalized");

        let mut tampered = complete_fixtures(DIGEST_B);
        tampered
            .get_mut("official-positive.json")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "description".to_owned(),
                JsonValue::String("tampered evidence".to_owned()),
            );
        let tampered_digest = snapshot_source_digest(&rule_set, &tampered).unwrap();
        assert_ne!(
            first, tampered_digest,
            "other fixture evidence remains hashed"
        );

        let mut tampered_context_sha = complete_fixtures(DIGEST_B);
        tampered_context_sha
            .get_mut("official-positive.json")
            .unwrap()
            .object_mut()
            .unwrap()
            .get_mut("input")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "context_fingerprint".to_owned(),
                JsonValue::String(DIGEST_B.to_owned()),
            );
        let tampered_context_digest =
            snapshot_source_digest(&rule_set, &tampered_context_sha).unwrap();
        assert_ne!(
            first, tampered_context_digest,
            "non-self-referential SHA fields remain hashed"
        );

        let document = reviewed_document();
        let error = validate_fixture_identity(&index(), &document, &complete_fixtures(DIGEST_B))
            .expect_err("tampered stored identity pin must fail");
        assert!(error.message().contains("source_set_sha256"));
    }
}
