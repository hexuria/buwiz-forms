//! Cross-platform structural audit of the legacy v1 evidence corpus under
//! `rules/`.
//!
//! This replaces `rules/validate.ps1` and `rules/validate-json-schema.ps1`,
//! which were only correct under Windows PowerShell: both partitioned the v2
//! trees with literal `\` separators, so on any other platform the exclusions
//! silently never matched and the wrong file set was audited.
//!
//! Every path here is resolved through [`crate::path`], which rejects `\`
//! outright, so the defect class cannot recur.
//!
//! The v2 IR is deliberately out of scope: it is audited by
//! [`crate::audit`], which enforces far stronger invariants.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{CodegenError, Result};
use crate::files::{json_files, read_tracked_bytes};
use crate::json::{JsonValue, parse_strict};
use crate::path::{canonical_repo_root, normalized_relative_path, resolve_existing_under};
use crate::schema::SchemaSet;

pub const DEFAULT_RULES_DIR: &str = "rules";

/// Corpus-relative prefixes owned by the v2 IR audit rather than this one.
const V2_PREFIXES: [&str; 2] = ["ir/v2/", "schema/v2/"];

/// Emitted so a consumer can tell which keyword subset validated the corpus.
/// The PowerShell validator advertised `tracked-json-schema-keyword-subset-v1`;
/// this validator reuses the strictly broader v2 `SchemaSet`, so the identifier
/// changes rather than silently claiming the narrower contract.
pub const V1_SCHEMA_VALIDATOR_ID: &str = "bir-rules-codegen-schema-set-v1";

#[derive(Clone, Debug)]
pub struct ValidateV1Options {
    pub repo_root: PathBuf,
    pub rules_dir: String,
}

impl ValidateV1Options {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            rules_dir: DEFAULT_RULES_DIR.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FormResult {
    pub form_id: String,
    pub fields: usize,
    pub validations: usize,
    pub calculations: usize,
    pub negative_fixtures: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct CorpusReport {
    /// Legacy (v1) JSON documents. Excludes both v2 trees.
    pub json_files: usize,
    pub total_json_files: usize,
    pub v2_json_files: usize,
    pub forms_audited: usize,
    pub fields: usize,
    pub validations: usize,
    pub calculations: usize,
    pub negative_fixtures: usize,
    pub schema_documents: usize,
    pub form_results: Vec<FormResult>,
    pub structural_audit: &'static str,
    pub json_schema_validation: &'static str,
    pub json_schema_validator: &'static str,
    pub v2_schema_validation: &'static str,
}

/// Audits the tracked v1 corpus and returns its reconciled counts.
///
/// Unlike the PowerShell original, JSON Schema validation is not optional and
/// has no "unavailable" fallback: a corpus that cannot be schema-validated is
/// an error, never a pass.
pub fn validate_v1(options: &ValidateV1Options) -> Result<CorpusReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let rules_root = resolve_existing_under(&repo_root, &options.rules_dir, "rules directory")?;

    let documents = load_documents(&rules_root)?;
    let total_json_files = documents.len();
    let legacy: BTreeMap<&String, &JsonValue> = documents
        .iter()
        .filter(|(relative, _)| !is_v2_path(relative))
        .collect();
    let legacy_count = legacy.len();

    validate_index(&documents)?;
    let form_results = audit_forms(&documents)?;
    let schema_documents = validate_schema_documents(&rules_root, &legacy)?;

    let fields = form_results.iter().map(|form| form.fields).sum();
    let validations = form_results.iter().map(|form| form.validations).sum();
    let calculations = form_results.iter().map(|form| form.calculations).sum();
    let negative_fixtures = form_results.iter().map(|form| form.negative_fixtures).sum();

    Ok(CorpusReport {
        json_files: legacy_count,
        total_json_files,
        v2_json_files: total_json_files - legacy_count,
        forms_audited: form_results.len(),
        fields,
        validations,
        calculations,
        negative_fixtures,
        schema_documents,
        form_results,
        structural_audit: "pass",
        json_schema_validation: "pass",
        json_schema_validator: V1_SCHEMA_VALIDATOR_ID,
        v2_schema_validation: "separate-bir-rules-codegen",
    })
}

fn is_v2_path(relative: &str) -> bool {
    V2_PREFIXES
        .iter()
        .any(|prefix| relative.starts_with(prefix))
}

/// True when any path component is a `schema` directory. Schema documents
/// describe instances; they are not themselves instances to validate.
fn is_under_schema_directory(relative: &str) -> bool {
    let mut components: Vec<&str> = relative.split('/').collect();
    components.pop();
    components.contains(&"schema")
}

fn load_documents(rules_root: &Path) -> Result<BTreeMap<String, JsonValue>> {
    let mut documents = BTreeMap::new();
    for path in json_files(rules_root)? {
        let relative = normalized_relative_path(rules_root, &path)?;
        let bytes = read_tracked_bytes(&path)?;
        let value = parse_strict(&bytes, &path).map_err(|source| {
            CodegenError::with_source(format!("invalid JSON: rules/{relative}"), source)
        })?;
        documents.insert(relative, value);
    }
    Ok(documents)
}

fn validate_index(documents: &BTreeMap<String, JsonValue>) -> Result<()> {
    let index = documents
        .get("index.json")
        .ok_or_else(|| CodegenError::new("missing rules index: rules/index.json"))?;
    let forms = array_at(index, "forms", "rules/index.json")?;
    let queue = array_at(index, "priority_queue", "rules/index.json")?;

    if queue.len() != forms.len() {
        return Err(CodegenError::new(
            "rules/index.json priority_queue length does not match forms length",
        ));
    }

    let mut form_ids = BTreeSet::new();
    let mut priorities = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut ordered: Vec<(u64, &JsonValue)> = Vec::with_capacity(forms.len());

    for entry in forms {
        let form_id = string_at(entry, "form_id", "rules/index.json entry")?;
        let path = string_at(entry, "path", "rules/index.json entry")?;
        let priority = integer_at(entry, "priority", "rules/index.json entry")?;
        if !form_ids.insert(form_id.to_owned()) {
            return Err(CodegenError::new(
                "rules/index.json has duplicate form_id entries",
            ));
        }
        if !priorities.insert(priority) {
            return Err(CodegenError::new(
                "rules/index.json has duplicate priority entries",
            ));
        }
        if !paths.insert(path.to_owned()) {
            return Err(CodegenError::new(
                "rules/index.json has duplicate manifest paths",
            ));
        }
        ordered.push((priority, entry));
    }

    ordered.sort_by_key(|(priority, _)| *priority);
    for (offset, (priority, entry)) in ordered.iter().enumerate() {
        let expected = offset as u64 + 1;
        if *priority != expected {
            return Err(CodegenError::new(format!(
                "rules/index.json priorities are not contiguous at {expected}"
            )));
        }
        let form_code = string_at(entry, "form_code", "rules/index.json entry")?;
        let queued = queue[offset].as_str().ok_or_else(|| {
            CodegenError::new(format!(
                "rules/index.json priority_queue entry {offset} is not a string"
            ))
        })?;
        if form_code != queued {
            return Err(CodegenError::new(format!(
                "rules/index.json priority_queue mismatch at priority {expected}"
            )));
        }

        let path = string_at(entry, "path", "rules/index.json entry")?;
        let manifest = documents.get(path).ok_or_else(|| {
            CodegenError::new(format!("indexed manifest is missing: rules/{path}"))
        })?;
        for key in ["form_id", "revision", "status"] {
            let indexed = string_at(entry, key, "rules/index.json entry")?;
            let declared = string_at(manifest, key, &format!("rules/{path}"))?;
            if indexed != declared {
                return Err(CodegenError::new(format!(
                    "indexed {key} does not match manifest: {indexed}"
                )));
            }
        }
    }
    Ok(())
}

fn audit_forms(documents: &BTreeMap<String, JsonValue>) -> Result<Vec<FormResult>> {
    let mut results = Vec::new();
    for form_dir in form_directories(documents) {
        let manifest_path = format!("{form_dir}/manifest.json");
        let Some(manifest) = documents.get(&manifest_path) else {
            continue;
        };

        let required = [
            ("fields", format!("{form_dir}/fields.json")),
            ("validations", format!("{form_dir}/validations.json")),
            ("calculations", format!("{form_dir}/calculations.json")),
            (
                "negative",
                format!("{form_dir}/fixtures/negative-cases.json"),
            ),
        ];
        let missing: Vec<&str> = required
            .iter()
            .filter(|(_, path)| !documents.contains_key(path))
            .map(|(_, path)| path.as_str())
            .collect();
        if !missing.is_empty() {
            let status = string_at(manifest, "status", &format!("rules/{manifest_path}"))?;
            if status == "complete" {
                let form_id = string_at(manifest, "form_id", &format!("rules/{manifest_path}"))?;
                return Err(CodegenError::new(format!(
                    "complete form {form_id} is missing required JSON: {}",
                    missing.join(", ")
                )));
            }
            continue;
        }

        let fields_doc = &documents[&required[0].1];
        let validations_doc = &documents[&required[1].1];
        let calculations_doc = &documents[&required[2].1];
        let negative_doc = &documents[&required[3].1];
        let prefix = string_at(manifest, "form_id", &format!("rules/{manifest_path}"))?.to_owned();

        let fields = array_at(fields_doc, "fields", &format!("rules/{}", required[0].1))?;
        let declared_field_count = integer_at(
            fields_doc,
            "field_count",
            &format!("rules/{}", required[0].1),
        )?;
        if declared_field_count != fields.len() as u64 {
            return Err(CodegenError::new(format!(
                "{prefix} fields.field_count does not match fields array length"
            )));
        }
        require_unique(fields, "field_key", &prefix, "field_key")?;

        let rules = array_at(
            validations_doc,
            "rules",
            &format!("rules/{}", required[1].1),
        )?;
        require_unique(rules, "rule_id", &prefix, "validation rule_id")?;

        let calculations = array_at(
            calculations_doc,
            "calculations",
            &format!("rules/{}", required[2].1),
        )?;
        require_unique(calculations, "calculation_id", &prefix, "calculation_id")?;

        let evaluation_order = array_at(
            calculations_doc,
            "evaluation_order",
            &format!("rules/{}", required[2].1),
        )?;
        let distinct_order: BTreeSet<&str> = evaluation_order
            .iter()
            .filter_map(JsonValue::as_str)
            .collect();
        if distinct_order.len() != calculations.len() {
            return Err(CodegenError::new(format!(
                "{prefix} calculation evaluation_order mismatch"
            )));
        }

        for entry in fields {
            require_source_refs(entry, "field_key", &prefix, "field")?;
        }
        for rule in rules {
            require_source_refs(rule, "rule_id", &prefix, "rule")?;
        }

        let counts = object_at(manifest, "counts", &format!("rules/{manifest_path}"))?;
        for (key, actual, label) in [
            ("typed_fields", fields.len(), "typed_fields"),
            ("validation_rules", rules.len(), "validation_rules"),
            ("calculations", calculations.len(), "calculations"),
        ] {
            let declared = counts.get(key).and_then(as_integer).ok_or_else(|| {
                CodegenError::new(format!(
                    "{prefix} manifest counts.{key} is missing or not an integer"
                ))
            })?;
            if declared != actual as u64 {
                return Err(CodegenError::new(format!(
                    "{prefix} manifest {label} count mismatch"
                )));
            }
        }

        let cases = array_at(negative_doc, "cases", &format!("rules/{}", required[3].1))?;
        let id_property = if cases
            .first()
            .and_then(JsonValue::object)
            .is_some_and(|first| first.contains_key("case_id"))
        {
            "case_id"
        } else {
            "id"
        };
        require_unique(
            cases,
            id_property,
            &prefix,
            &format!("negative fixture {id_property}"),
        )?;

        let rule_ids: BTreeSet<&str> = rules
            .iter()
            .filter_map(|rule| rule.object()?.get("rule_id")?.as_str())
            .collect();
        for case in cases {
            let case_id = string_at(case, id_property, &format!("rules/{}", required[3].1))?;
            let rule_id = case
                .object()
                .and_then(|case| case.get("rule_id"))
                .and_then(JsonValue::as_str)
                .unwrap_or_default();
            if rule_id.is_empty() || !rule_ids.contains(rule_id) {
                return Err(CodegenError::new(format!(
                    "{prefix} negative fixture has unknown rule_id: {case_id}: {rule_id}"
                )));
            }
        }

        results.push(FormResult {
            form_id: prefix,
            fields: fields.len(),
            validations: rules.len(),
            calculations: calculations.len(),
            negative_fixtures: cases.len(),
        });
    }
    Ok(results)
}

/// `forms/<name>` prefixes, in sorted order. Derived from the already-walked
/// document set so the corpus is only traversed once.
fn form_directories(documents: &BTreeMap<String, JsonValue>) -> Vec<String> {
    let mut directories = BTreeSet::new();
    for relative in documents.keys() {
        let mut components = relative.split('/');
        if components.next() != Some("forms") {
            continue;
        }
        if let Some(name) = components.next() {
            directories.insert(format!("forms/{name}"));
        }
    }
    directories.into_iter().collect()
}

/// Validates every legacy instance document that declares a `$schema`, against
/// the schema it names. Returns the number validated.
fn validate_schema_documents(
    rules_root: &Path,
    legacy: &BTreeMap<&String, &JsonValue>,
) -> Result<usize> {
    let schema_root = resolve_existing_under(rules_root, "schema", "v1 schema directory")?;
    let schemas = SchemaSet::load_top_level_tracked(&schema_root)?;

    let mut validated = 0usize;
    for (relative, document) in legacy {
        if is_under_schema_directory(relative) {
            continue;
        }
        let Some(pointer) = document
            .object()
            .and_then(|document| document.get("$schema"))
            .and_then(JsonValue::as_str)
        else {
            continue;
        };

        let schema_name = resolve_schema_name(rules_root, relative, pointer)?;
        schemas.validate(&schema_name, document).map_err(|source| {
            CodegenError::with_source(
                format!("schema validation failed: rules/{relative}"),
                source,
            )
        })?;
        validated += 1;
    }
    Ok(validated)
}

/// Resolves a document-relative `$schema` pointer to a file name directly under
/// `rules/schema/`. Rejects anything that escapes that directory.
fn resolve_schema_name(rules_root: &Path, relative: &str, pointer: &str) -> Result<String> {
    if pointer.contains('\\') {
        return Err(CodegenError::new(format!(
            "rules/{relative} declares a non-portable $schema `{pointer}`"
        )));
    }

    let mut components: Vec<&str> = relative.split('/').collect();
    components.pop();
    for component in pointer.split('/') {
        match component {
            "." | "" => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(CodegenError::new(format!(
                        "rules/{relative} declares a $schema `{pointer}` outside the corpus"
                    )));
                }
            }
            other => components.push(other),
        }
    }

    let resolved = components.join("/");
    let name = resolved.strip_prefix("schema/").ok_or_else(|| {
        CodegenError::new(format!(
            "rules/{relative} declares a $schema `{pointer}` outside `rules/schema/`"
        ))
    })?;
    if name.contains('/') {
        return Err(CodegenError::new(format!(
            "rules/{relative} declares a nested $schema `{pointer}`"
        )));
    }
    // Proves the schema exists on disk with the same portability guarantees as
    // every other resolved path.
    resolve_existing_under(rules_root, &resolved, "declared v1 schema")?;
    Ok(name.to_owned())
}

fn require_unique(entries: &[JsonValue], key: &str, prefix: &str, label: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for entry in entries {
        let value = entry
            .object()
            .and_then(|entry| entry.get(key))
            .and_then(JsonValue::as_str)
            .ok_or_else(|| CodegenError::new(format!("{prefix} has an entry without {key}")))?;
        if !seen.insert(value) {
            return Err(CodegenError::new(format!("{prefix} has duplicate {label}")));
        }
    }
    Ok(())
}

fn require_source_refs(entry: &JsonValue, id_key: &str, prefix: &str, label: &str) -> Result<()> {
    let identifier = entry
        .object()
        .and_then(|entry| entry.get(id_key))
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let populated = entry
        .object()
        .and_then(|entry| entry.get("source_refs"))
        .is_some_and(|refs| matches!(refs, JsonValue::Array(values) if !values.is_empty()));
    if !populated {
        return Err(CodegenError::new(format!(
            "{prefix} {label} without source_refs: {identifier}"
        )));
    }
    Ok(())
}

fn array_at<'a>(value: &'a JsonValue, key: &str, context: &str) -> Result<&'a Vec<JsonValue>> {
    match value.object().and_then(|object| object.get(key)) {
        Some(JsonValue::Array(values)) => Ok(values),
        _ => Err(CodegenError::new(format!(
            "{context} is missing array `{key}`"
        ))),
    }
}

fn object_at<'a>(
    value: &'a JsonValue,
    key: &str,
    context: &str,
) -> Result<&'a BTreeMap<String, JsonValue>> {
    value
        .object()
        .and_then(|object| object.get(key))
        .and_then(JsonValue::object)
        .ok_or_else(|| CodegenError::new(format!("{context} is missing object `{key}`")))
}

fn string_at<'a>(value: &'a JsonValue, key: &str, context: &str) -> Result<&'a str> {
    value
        .object()
        .and_then(|object| object.get(key))
        .and_then(JsonValue::as_str)
        .ok_or_else(|| CodegenError::new(format!("{context} is missing string `{key}`")))
}

fn integer_at(value: &JsonValue, key: &str, context: &str) -> Result<u64> {
    value
        .object()
        .and_then(|object| object.get(key))
        .and_then(as_integer)
        .ok_or_else(|| CodegenError::new(format!("{context} is missing integer `{key}`")))
}

fn as_integer(value: &JsonValue) -> Option<u64> {
    match value {
        JsonValue::Number(number) => number.as_u64(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ValidateV1Options, is_under_schema_directory, is_v2_path, resolve_schema_name, validate_v1,
    };
    use crate::path::canonical_repo_root;

    fn landed_repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// Locks the published corpus baseline.
    ///
    /// These are the counts the Windows PowerShell validator reported, and they
    /// are quoted as fact in `handoff.md`, `rules/forms/2550q-v2024/audit.md`
    /// and `docs/validation-rules/implementation-plan.md`. If this test fails,
    /// either the corpus changed or the port is wrong. Never adjust the
    /// expected values to make it pass.
    #[test]
    fn landed_v1_corpus_reconciles_its_published_counts() {
        let report =
            validate_v1(&ValidateV1Options::new(landed_repo_root())).expect("audit v1 corpus");

        assert_eq!(report.forms_audited, 43, "forms");
        assert_eq!(report.total_json_files, 659, "total JSON files");
        assert_eq!(report.json_files, 520, "legacy JSON files");
        assert_eq!(report.v2_json_files, 139, "v2 JSON files");
        assert_eq!(report.fields, 9_592, "fields");
        assert_eq!(report.validations, 2_007, "validations");
        assert_eq!(report.calculations, 623, "calculations");
        assert_eq!(report.negative_fixtures, 1_354, "negative fixtures");
        assert_eq!(report.schema_documents, 216, "schema documents");
        assert_eq!(report.structural_audit, "pass");
        assert_eq!(report.json_schema_validation, "pass");
        assert_eq!(report.form_results.len(), report.forms_audited);
    }

    /// The v1 audit must never claim authority over the v2 IR.
    #[test]
    fn v1_audit_leaves_v2_validation_to_the_codegen_audit() {
        let report =
            validate_v1(&ValidateV1Options::new(landed_repo_root())).expect("audit v1 corpus");
        assert_eq!(report.v2_schema_validation, "separate-bir-rules-codegen");
        assert_eq!(
            report.json_files + report.v2_json_files,
            report.total_json_files
        );
    }

    #[test]
    fn v2_trees_are_excluded_by_forward_slash_prefixes() {
        // The PowerShell original compared against `ir\v2\`, so on macOS and
        // Linux nothing matched and the v2 corpus was audited as legacy.
        assert!(is_v2_path("ir/v2/2550q-v2024-p7.9.6.0/rule-set.json"));
        assert!(is_v2_path("schema/v2/common.schema.json"));
        assert!(!is_v2_path("forms/2550q-v2024/fields.json"));
        assert!(!is_v2_path("schema/fields.schema.json"));
        assert!(!is_v2_path("index.json"));
    }

    #[test]
    fn schema_directory_detection_ignores_the_file_name() {
        assert!(is_under_schema_directory("schema/fields.schema.json"));
        assert!(is_under_schema_directory("schema/v2/common.schema.json"));
        assert!(!is_under_schema_directory("forms/0605-v2003/fields.json"));
        // A document merely named like a schema is still an instance.
        assert!(!is_under_schema_directory("forms/x/my.schema.json"));
    }

    #[test]
    fn schema_pointers_resolve_relative_to_the_declaring_document() {
        let root = canonical_repo_root(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("crate root")
            .join("../../rules");
        let Ok(root) = std::fs::canonicalize(root) else {
            return; // corpus not present in this checkout
        };
        let name = resolve_schema_name(
            &root,
            "forms/2550q-v2024/fields.json",
            "../../schema/fields.schema.json",
        )
        .expect("resolves");
        assert_eq!(name, "fields.schema.json");
    }

    #[test]
    fn schema_pointers_cannot_escape_the_schema_directory() {
        let root = std::path::Path::new("/nonexistent");
        assert!(resolve_schema_name(root, "forms/x/fields.json", "../../index.json").is_err());
        assert!(resolve_schema_name(root, "forms/x/fields.json", "../../../outside.json").is_err());
        assert!(resolve_schema_name(root, "forms/x/fields.json", r"..\..\schema\a.json").is_err());
    }
}
