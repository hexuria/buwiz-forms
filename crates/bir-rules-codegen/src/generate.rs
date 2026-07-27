use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::audit::{
    AuditOptions, AuditReport, AuditedSnapshot, audit, validate_boolean_coercion_tokens,
    validate_candidate_readiness, validate_reviewed_evaluation_policy,
};
use crate::emit::render_static_rule_set;
use crate::error::{CodegenError, Result};
use crate::files::write_tree_atomically;
use crate::hash::{digest_entries, sha256_hex};
use crate::json::{CANONICALIZATION_ID, JsonValue, canonical_bytes};
use crate::model::ReviewStatus;
use crate::path::{DEFAULT_OUTPUT_DIR, resolve_output_under};

pub const MANIFEST_FORMAT: &str = "bir-rules-generated-manifest-v1";

#[derive(Clone, Debug)]
pub struct GenerateOptions {
    pub audit: AuditOptions,
    pub output_dir: String,
    pub required_rule_set_id: Option<String>,
}

impl GenerateOptions {
    pub fn tracked_checkout(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            audit: AuditOptions::tracked_checkout(repo_root),
            output_dir: DEFAULT_OUTPUT_DIR.to_owned(),
            required_rule_set_id: None,
        }
    }

    pub fn external_workspace(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            audit: AuditOptions::external_workspace(repo_root),
            output_dir: DEFAULT_OUTPUT_DIR.to_owned(),
            required_rule_set_id: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GenerationReport {
    pub schema_digest: String,
    pub normalized_source_digest: String,
    pub generated_output_digest: String,
    pub manifest_digest: String,
    pub reviewed_snapshot_count: usize,
    pub candidate_snapshot_count: usize,
    pub files: BTreeMap<String, Vec<u8>>,
}

pub fn generate(options: &GenerateOptions) -> Result<GenerationReport> {
    let audit_report = audit_for_generation(options)?;
    let generation = build_generated_files(&audit_report)?;
    let output = resolve_generated_output(&audit_report.repo_root, &options.output_dir)?;
    let write = write_tree_atomically(&output, &generation.files)?;
    if let Some(previous) = write.preserved_previous {
        eprintln!(
            "previous generated output was preserved at `{}`; review it before manual removal",
            previous.display()
        );
    }
    Ok(generation)
}

/// Performs the complete aggregate audit before asserting an optional focus
/// identity. The returned report always retains every audited snapshot.
pub(crate) fn audit_for_generation(options: &GenerateOptions) -> Result<AuditReport> {
    let report = audit(&options.audit)?;
    require_generation_rule_set(&report, options.required_rule_set_id.as_deref())?;
    Ok(report)
}

fn require_generation_rule_set(
    report: &AuditReport,
    required_rule_set_id: Option<&str>,
) -> Result<()> {
    if let Some(rule_set_id) = required_rule_set_id {
        report.require_rule_set(rule_set_id)?;
    }
    Ok(())
}

pub(crate) fn resolve_generated_output(
    repo_root: &std::path::Path,
    output_dir: &str,
) -> Result<PathBuf> {
    if output_dir != DEFAULT_OUTPUT_DIR {
        return Err(CodegenError::new(format!(
            "generated output is locked to `{DEFAULT_OUTPUT_DIR}`, not `{output_dir}`"
        )));
    }
    resolve_output_under(repo_root, output_dir, "generated output directory")
}

/// Builds an output tree only from a report created and retained inside this
/// crate. Public callers must use [`generate`], which performs its own audit.
pub(crate) fn build_generated_files(audit: &AuditReport) -> Result<GenerationReport> {
    let mut emitted = audit
        .snapshots
        .iter()
        .filter(|snapshot| snapshot.document.review_status != ReviewStatus::Skeleton)
        .collect::<Vec<_>>();
    emitted.sort_by(|left, right| {
        let left = &left.document.identity;
        let right = &right.document.identity;
        (
            &left.rule_set_id,
            &left.form_code,
            &left.form_revision,
            &left.official_package_version,
            &left.source_set_sha256,
        )
            .cmp(&(
                &right.rule_set_id,
                &right.form_code,
                &right.form_revision,
                &right.official_package_version,
                &right.source_set_sha256,
            ))
    });

    let mut module_names = BTreeSet::new();
    let mut reviewed_modules = Vec::new();
    let mut candidate_modules = Vec::new();
    let mut files = BTreeMap::new();
    for snapshot in &emitted {
        match snapshot.document.review_status {
            ReviewStatus::Candidate => {
                validate_candidate_readiness(&snapshot.document, &snapshot.fixtures)?;
            }
            ReviewStatus::Reviewed => validate_reviewed_evaluation_policy(&snapshot.document)?,
            ReviewStatus::Skeleton => unreachable!("skeleton snapshots are not emitted"),
        }
        validate_boolean_coercion_tokens(&snapshot.document)?;
        let module = module_name(snapshot);
        if !module_names.insert(module.clone()) {
            return Err(CodegenError::new(format!(
                "emitted snapshot module-name collision at `{module}`"
            )));
        }
        let file_name = format!("{module}.rs");
        files.insert(file_name, render_snapshot_module(snapshot)?.into_bytes());
        match snapshot.document.review_status {
            ReviewStatus::Candidate => candidate_modules.push((module, *snapshot)),
            ReviewStatus::Reviewed => reviewed_modules.push((module, *snapshot)),
            ReviewStatus::Skeleton => unreachable!("skeleton snapshots are not emitted"),
        }
    }

    files.insert(
        "mod.rs".to_owned(),
        render_mod_rs(&reviewed_modules, &candidate_modules).into_bytes(),
    );
    files.insert(
        "registry.rs".to_owned(),
        render_registry_rs(&reviewed_modules, &candidate_modules).into_bytes(),
    );
    format_rust_files(&audit.repo_root, &mut files)?;

    let generated_output_digest = digest_file_tree("bir-rules-generated-files-v1", &files);
    let generator_digest = generator_source_digest();
    let manifest = render_manifest(
        audit,
        &files,
        &generated_output_digest,
        &generator_digest,
        &reviewed_modules,
        &candidate_modules,
    );
    let manifest_digest = sha256_hex(&manifest);
    files.insert("manifest.json".to_owned(), manifest);

    Ok(GenerationReport {
        schema_digest: audit.schema_digest.clone(),
        normalized_source_digest: audit.normalized_source_digest.clone(),
        generated_output_digest,
        manifest_digest,
        reviewed_snapshot_count: reviewed_modules.len(),
        candidate_snapshot_count: candidate_modules.len(),
        files,
    })
}

fn format_rust_files(
    repo_root: &std::path::Path,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let config_path = repo_root.join(".rustfmt.toml");
    for (path, bytes) in files.iter_mut().filter(|(path, _)| path.ends_with(".rs")) {
        let mut child = Command::new(rustfmt_executable())
            .current_dir(repo_root)
            .args(["--edition", "2024", "--emit", "stdout", "--config-path"])
            .arg(&config_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| {
                CodegenError::with_source(
                    &format!("failed to start rustfmt for generated `{path}`"),
                    source,
                )
            })?;
        child
            .stdin
            .take()
            .expect("piped rustfmt stdin")
            .write_all(bytes)
            .map_err(|source| {
                CodegenError::with_source(&format!("write generated `{path}` to rustfmt"), source)
            })?;
        let output = child.wait_with_output().map_err(|source| {
            CodegenError::with_source(&format!("wait for rustfmt on generated `{path}`"), source)
        })?;
        if !output.status.success() {
            return Err(CodegenError::new(format!(
                "rustfmt failed for generated `{path}` with {}{}{}",
                output.status,
                command_output("stdout", &output.stdout),
                command_output("stderr", &output.stderr),
            )));
        }
        *bytes = output.stdout;
    }
    Ok(())
}

fn rustfmt_executable() -> OsString {
    std::env::var_os("RUSTFMT").unwrap_or_else(|| "rustfmt".into())
}

fn command_output(label: &str, bytes: &[u8]) -> String {
    if bytes.is_empty() {
        String::new()
    } else {
        format!("\n{label}:\n{}", String::from_utf8_lossy(bytes))
    }
}

fn render_mod_rs(
    reviewed_modules: &[(String, &AuditedSnapshot)],
    candidate_modules: &[(String, &AuditedSnapshot)],
) -> String {
    let mut output = generated_banner();
    output.push_str("mod registry;\n");
    for (module, _) in reviewed_modules {
        output.push_str(&format!("mod {module};\n"));
    }
    for (module, _) in candidate_modules {
        output.push_str(&format!("#[cfg(test)]\nmod {module};\n"));
    }
    output.push_str(
        "\npub use registry::{\n\
         \x20   GeneratedRuleSetMetadata, REVIEWED_RULE_SET_METADATA, reviewed_rule_set_entries,\n\
         };\n",
    );
    if !candidate_modules.is_empty() {
        output.push_str(
            "\n#[cfg(test)]\n\
             pub use registry::{\n\
             \x20   CANDIDATE_RULE_SET_METADATA, candidate_rule_set_entries,\n\
             };\n",
        );
    }
    output
}

fn render_registry_rs(
    snapshot_modules: &[(String, &AuditedSnapshot)],
    candidate_modules: &[(String, &AuditedSnapshot)],
) -> String {
    let mut output = generated_banner();
    output.push_str(
        "use crate::RuleSetRegistryEntry;\n\
         use std::sync::LazyLock;\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct GeneratedRuleSetMetadata {\n\
         \x20   pub rule_set_id: &'static str,\n\
         \x20   pub form_code: &'static str,\n\
         \x20   pub form_revision: &'static str,\n\
         \x20   pub official_package_version: &'static str,\n\
         \x20   pub source_set_sha256: &'static str,\n\
         \x20   pub canonical_rule_set_json: &'static str,\n\
         }\n\n",
    );
    if snapshot_modules.is_empty() {
        output.push_str(
            "pub static REVIEWED_RULE_SET_METADATA: &[GeneratedRuleSetMetadata] = &[];\n\n",
        );
    } else {
        output
            .push_str("pub static REVIEWED_RULE_SET_METADATA: &[GeneratedRuleSetMetadata] = &[\n");
        for (module, _) in snapshot_modules {
            output.push_str("    GeneratedRuleSetMetadata {\n");
            output.push_str(&format!(
                "        rule_set_id: super::{module}::RULE_SET_ID,\n\
                 \x20       form_code: super::{module}::FORM_CODE,\n\
                 \x20       form_revision: super::{module}::FORM_REVISION,\n\
                 \x20       official_package_version: super::{module}::OFFICIAL_PACKAGE_VERSION,\n\
                 \x20       source_set_sha256: super::{module}::SOURCE_SET_SHA256,\n\
                 \x20       canonical_rule_set_json: super::{module}::CANONICAL_RULE_SET_JSON,\n"
            ));
            output.push_str("    },\n");
        }
        output.push_str("];\n\n");
    }
    if snapshot_modules.is_empty() {
        output.push_str(
            "static REVIEWED_RULE_SET_ENTRIES: LazyLock<Vec<RuleSetRegistryEntry>> = \
             LazyLock::new(|| vec![]);\n\n",
        );
    } else {
        output.push_str(
            "static REVIEWED_RULE_SET_ENTRIES: LazyLock<Vec<RuleSetRegistryEntry>> =\n\
             \x20   LazyLock::new(|| vec![\n",
        );
        for (module, _) in snapshot_modules {
            output.push_str(&format!(
                "        RuleSetRegistryEntry::new(&*super::{module}::COMPILED_RULE_SET),\n"
            ));
        }
        output.push_str("    ]);\n\n");
    }
    output.push_str(
        "pub fn reviewed_rule_set_entries() -> &'static [RuleSetRegistryEntry] {\n\
         \x20   REVIEWED_RULE_SET_ENTRIES.as_slice()\n\
         }\n",
    );
    if !candidate_modules.is_empty() {
        output.push_str(
            "\n// Candidate providers exist for library verification only. Both the modules and\n\
             // this catalog are absent from non-test builds.\n\
             #[cfg(test)]\n\
             pub static CANDIDATE_RULE_SET_METADATA: &[GeneratedRuleSetMetadata] = &[\n",
        );
        for (module, _) in candidate_modules {
            output.push_str("    GeneratedRuleSetMetadata {\n");
            output.push_str(&format!(
                "        rule_set_id: super::{module}::RULE_SET_ID,\n\
                 \x20       form_code: super::{module}::FORM_CODE,\n\
                 \x20       form_revision: super::{module}::FORM_REVISION,\n\
                 \x20       official_package_version: super::{module}::OFFICIAL_PACKAGE_VERSION,\n\
                 \x20       source_set_sha256: super::{module}::SOURCE_SET_SHA256,\n\
                 \x20       canonical_rule_set_json: super::{module}::CANONICAL_RULE_SET_JSON,\n"
            ));
            output.push_str("    },\n");
        }
        output.push_str(
            "];\n\n\
             #[cfg(test)]\n\
             static CANDIDATE_RULE_SET_ENTRIES: LazyLock<Vec<RuleSetRegistryEntry>> =\n\
             \x20   LazyLock::new(|| vec![\n",
        );
        for (module, _) in candidate_modules {
            output.push_str(&format!(
                "        RuleSetRegistryEntry::new(&*super::{module}::COMPILED_RULE_SET),\n"
            ));
        }
        output.push_str(
            "    ]);\n\n\
             #[cfg(test)]\n\
             pub fn candidate_rule_set_entries() -> &'static [RuleSetRegistryEntry] {\n\
             \x20   CANDIDATE_RULE_SET_ENTRIES.as_slice()\n\
             }\n",
        );
    }
    output
}

fn render_snapshot_module(snapshot: &AuditedSnapshot) -> Result<String> {
    let identity = &snapshot.document.identity;
    let source_digest = identity.source_set_sha256.as_deref().ok_or_else(|| {
        CodegenError::new(format!(
            "cannot emit {} snapshot `{}` without source_set_sha256",
            review_status_label(&snapshot.document.review_status),
            identity.rule_set_id,
        ))
    })?;
    let canonical = String::from_utf8(snapshot.canonical_rule_set.clone())
        .expect("canonical JSON is valid UTF-8");
    let mut output = generated_banner();
    output.push_str(
        "use crate::serialization::{\n\
         \x20   AbsentValuePolicy, BlankValuePolicy, BodyCodec, ExactDatePattern,\n\
         \x20   SerializationArtifactTarget,\n\
         };\n\
         use crate::serialization_contract::*;\n\
         use crate::static_ir::*;\n\
         use crate::{\n\
         \x20   FormRevisionKey, RuleAssessment, RuleSeverity, ValidationPhase, WorkflowAction,\n\
         \x20   WorkflowNotificationChannel,\n\
         };\n\
         use std::sync::LazyLock;\n\n",
    );
    output.push_str(&format!(
        "pub const RULE_SET_ID: &str = {};\n\
         pub const FORM_CODE: &str = {};\n\
         pub const FORM_REVISION: &str = {};\n\
         pub const OFFICIAL_PACKAGE_VERSION: &str = {};\n\
         pub const SOURCE_SET_SHA256: &str = {};\n\
         pub const CANONICAL_RULE_SET_JSON: &str = {};\n",
        rust_string(&identity.rule_set_id),
        rust_string(&identity.form_code),
        rust_string(&identity.form_revision),
        rust_string(&identity.official_package_version),
        rust_string(source_digest),
        rust_raw_string(&canonical),
    ));
    output.push('\n');
    output.push_str(&render_static_rule_set(snapshot)?);
    output.push_str(&render_evaluation_fixture_tests(snapshot)?);
    Ok(output)
}

fn render_evaluation_fixture_tests(snapshot: &AuditedSnapshot) -> Result<String> {
    let rule_set_id = &snapshot.document.identity.rule_set_id;
    let status = review_status_label(&snapshot.document.review_status);
    if snapshot.fixtures.is_empty() {
        return Err(CodegenError::new(format!(
            "cannot emit {status} snapshot `{rule_set_id}` without concrete evaluation fixtures"
        )));
    }

    let mut fixtures = Vec::with_capacity(snapshot.fixtures.len());
    for (path, fixture) in &snapshot.fixtures {
        let fixture = fixture.object().ok_or_else(|| {
            CodegenError::new(format!(
                "cannot emit {status} snapshot `{rule_set_id}`: fixture `{path}` is not an object"
            ))
        })?;
        let fixture_id = fixture
            .get("fixture_id")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "cannot emit {status} snapshot `{rule_set_id}`: fixture `{path}` has no string fixture_id"
                ))
            })?;
        let kind = fixture
            .get("kind")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "cannot emit {status} snapshot `{rule_set_id}`: fixture `{path}` has no string kind"
                ))
            })?;
        if kind != "evaluation" {
            return Err(CodegenError::new(format!(
                "cannot emit {status} snapshot `{rule_set_id}`: fixture `{fixture_id}` at `{path}` has non-evaluation kind `{kind}`"
            )));
        }
        let input = fixture.get("input").ok_or_else(|| {
            CodegenError::new(format!(
                "cannot emit {status} snapshot `{rule_set_id}`: evaluation fixture `{fixture_id}` at `{path}` has no input"
            ))
        })?;
        if input.object().is_none() {
            return Err(CodegenError::new(format!(
                "cannot emit {status} snapshot `{rule_set_id}`: evaluation fixture `{fixture_id}` at `{path}` input is not an object"
            )));
        }
        let expected = fixture.get("expected").ok_or_else(|| {
            CodegenError::new(format!(
                "cannot emit {status} snapshot `{rule_set_id}`: evaluation fixture `{fixture_id}` at `{path}` has no expected result"
            ))
        })?;
        if expected.object().is_none() {
            return Err(CodegenError::new(format!(
                "cannot emit {status} snapshot `{rule_set_id}`: evaluation fixture `{fixture_id}` at `{path}` expected result is not an object"
            )));
        }

        let workflow_transition = fixture
            .get("workflow_transition")
            .map(|value| {
                let object = value.object().ok_or_else(|| {
                    CodegenError::new(format!(
                        "cannot emit {status} snapshot `{rule_set_id}`: evaluation fixture `{fixture_id}` workflow_transition is not an object"
                    ))
                })?;
                let current_state = object
                    .get("current_state")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| {
                        CodegenError::new(format!(
                            "cannot emit {status} snapshot `{rule_set_id}`: evaluation fixture `{fixture_id}` workflow_transition has no current_state"
                        ))
                    })?;
                let action = object
                    .get("action")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| {
                        CodegenError::new(format!(
                            "cannot emit {status} snapshot `{rule_set_id}`: evaluation fixture `{fixture_id}` workflow_transition has no action"
                        ))
                    })?;
                let action = match action {
                    "edit" => "WorkflowAction::Edit",
                    "save" => "WorkflowAction::Save",
                    "validate" => "WorkflowAction::Validate",
                    "final-copy" => "WorkflowAction::FinalCopy",
                    "submit" => "WorkflowAction::Submit",
                    "print-preview" => "WorkflowAction::PrintPreview",
                    other => {
                        return Err(CodegenError::new(format!(
                            "cannot emit {status} snapshot `{rule_set_id}`: evaluation fixture `{fixture_id}` has unsupported workflow action `{other}`"
                        )));
                    }
                };
                let expected = object.get("expected").ok_or_else(|| {
                    CodegenError::new(format!(
                        "cannot emit {status} snapshot `{rule_set_id}`: evaluation fixture `{fixture_id}` workflow_transition has no expected result"
                    ))
                })?;
                Ok((
                    current_state.to_owned(),
                    action,
                    String::from_utf8(canonical_bytes(expected))
                        .expect("canonical workflow result is UTF-8"),
                ))
            })
            .transpose()?;

        fixtures.push((
            fixture_id,
            String::from_utf8(canonical_bytes(input)).expect("canonical fixture input is UTF-8"),
            String::from_utf8(canonical_bytes(expected))
                .expect("canonical fixture expected result is UTF-8"),
            workflow_transition,
        ));
    }

    let mut output = String::from(
        "\n#[cfg(test)]\n\
         mod evaluation_fixture_tests {\n\
         \x20   use super::COMPILED_RULE_SET;\n\
         \x20   use crate::{\n\
         \x20       CompiledRuleSet, EvaluationRequest, EvaluationResult, WorkflowAction,\n\
         \x20       WorkflowStateId, WorkflowTransitionResult,\n\
         \x20   };\n\n\
         \x20   struct EvaluationFixture {\n\
         \x20       fixture_id: &'static str,\n\
         \x20       input_json: &'static str,\n\
         \x20       expected_json: &'static str,\n\
         \x20       workflow_current_state: Option<&'static str>,\n\
         \x20       workflow_action: Option<WorkflowAction>,\n\
         \x20       expected_workflow_json: Option<&'static str>,\n\
         \x20   }\n\n\
         \x20   const EVALUATION_FIXTURES: &[EvaluationFixture] = &[\n",
    );
    for (fixture_id, input_json, expected_json, workflow_transition) in fixtures {
        output.push_str("        EvaluationFixture {\n");
        output.push_str(&format!(
            "            fixture_id: {},\n",
            rust_string(fixture_id)
        ));
        output.push_str(&format!(
            "            input_json: {},\n",
            rust_raw_string(&input_json)
        ));
        output.push_str(&format!(
            "            expected_json: {},\n",
            rust_raw_string(&expected_json)
        ));
        match workflow_transition {
            Some((current_state, action, expected_workflow_json)) => {
                output.push_str(&format!(
                    "            workflow_current_state: Some({}),\n",
                    rust_string(&current_state)
                ));
                output.push_str(&format!("            workflow_action: Some({action}),\n"));
                output.push_str(&format!(
                    "            expected_workflow_json: Some({}),\n",
                    rust_raw_string(&expected_workflow_json)
                ));
            }
            None => {
                output.push_str("            workflow_current_state: None,\n");
                output.push_str("            workflow_action: None,\n");
                output.push_str("            expected_workflow_json: None,\n");
            }
        }
        output.push_str("        },\n");
    }
    output.push_str(
         "    ];\n\n\
         \x20   #[test]\n\
         \x20   fn evaluation_fixtures_match_compiled_provider() {\n\
         \x20       for fixture in EVALUATION_FIXTURES {\n\
         \x20           let request: EvaluationRequest =\n\
         \x20               serde_json::from_str(fixture.input_json).unwrap_or_else(|error| {\n\
         \x20                   panic!(\n\
         \x20                       \"evaluation fixture `{}` input failed to deserialize: {error}\",\n\
         \x20                       fixture.fixture_id\n\
         \x20                   )\n\
         \x20               });\n\
         \x20           let expected: EvaluationResult =\n\
         \x20               serde_json::from_str(fixture.expected_json).unwrap_or_else(|error| {\n\
         \x20                   panic!(\n\
         \x20                       \"evaluation fixture `{}` expected result failed to deserialize: {error}\",\n\
         \x20                       fixture.fixture_id\n\
         \x20                   )\n\
         \x20               });\n\
         \x20           let actual = CompiledRuleSet::evaluate(&*COMPILED_RULE_SET, &request)\n\
         \x20               .unwrap_or_else(|error| {\n\
         \x20                   panic!(\n\
         \x20                       \"evaluation fixture `{}` failed to evaluate: {error}\",\n\
         \x20                       fixture.fixture_id\n\
         \x20                   )\n\
         \x20               });\n\
         \x20           assert_eq!(\n\
         \x20               actual, expected,\n\
         \x20               \"compiled provider result differs from evaluation fixture `{}`\",\n\
         \x20               fixture.fixture_id\n\
         \x20           );\n\
         \x20           match (\n\
         \x20               fixture.workflow_current_state,\n\
         \x20               fixture.workflow_action,\n\
         \x20               fixture.expected_workflow_json,\n\
         \x20           ) {\n\
         \x20               (Some(current_state), Some(action), Some(expected_json)) => {\n\
         \x20                   let current_state = WorkflowStateId::parse(current_state)\n\
         \x20                       .expect(\"audited workflow fixture current state\");\n\
         \x20                   let expected: WorkflowTransitionResult =\n\
         \x20                       serde_json::from_str(expected_json).unwrap_or_else(|error| {\n\
         \x20                           panic!(\n\
         \x20                               \"evaluation fixture `{}` expected workflow result failed to deserialize: {error}\",\n\
         \x20                               fixture.fixture_id\n\
         \x20                           )\n\
         \x20                       });\n\
         \x20                   let actual = CompiledRuleSet::transition_workflow(\n\
         \x20                       &*COMPILED_RULE_SET,\n\
         \x20                       &request,\n\
         \x20                       &actual,\n\
         \x20                       &current_state,\n\
         \x20                       action,\n\
         \x20                   )\n\
         \x20                   .unwrap_or_else(|error| {\n\
         \x20                       panic!(\n\
         \x20                           \"evaluation fixture `{}` failed workflow transition: {error}\",\n\
         \x20                           fixture.fixture_id\n\
         \x20                       )\n\
         \x20                   });\n\
         \x20                   assert_eq!(\n\
         \x20                       actual, expected,\n\
         \x20                       \"compiled workflow result differs from evaluation fixture `{}`\",\n\
         \x20                       fixture.fixture_id\n\
         \x20                   );\n\
         \x20               }\n\
         \x20               (None, None, None) => {}\n\
         \x20               _ => panic!(\"evaluation fixture `{}` has incomplete generated workflow data\", fixture.fixture_id),\n\
         \x20           }\n\
         \x20       }\n\
         \x20   }\n\
         }\n",
    );
    Ok(output)
}

fn render_manifest(
    audit: &AuditReport,
    generated_files: &BTreeMap<String, Vec<u8>>,
    output_digest: &str,
    generator_digest: &str,
    reviewed_modules: &[(String, &AuditedSnapshot)],
    candidate_modules: &[(String, &AuditedSnapshot)],
) -> Vec<u8> {
    let generated_modules = reviewed_modules
        .iter()
        .chain(candidate_modules)
        .map(|(module, snapshot)| (snapshot.index.rule_set_id.as_str(), module.as_str()))
        .collect::<BTreeMap<_, _>>();

    let snapshots = audit
        .snapshots
        .iter()
        .map(|snapshot| {
            object([
                (
                    "generated_module",
                    generated_modules
                        .get(snapshot.index.rule_set_id.as_str())
                        .map(|module| JsonValue::String((*module).to_owned()))
                        .unwrap_or(JsonValue::Null),
                ),
                (
                    "normalized_source_sha256",
                    JsonValue::String(snapshot.normalized_source_sha256.clone()),
                ),
                (
                    "review_status",
                    JsonValue::String(match snapshot.document.review_status {
                        ReviewStatus::Skeleton => "skeleton".to_owned(),
                        ReviewStatus::Candidate => "candidate".to_owned(),
                        ReviewStatus::Reviewed => "reviewed".to_owned(),
                    }),
                ),
                (
                    "rule_set_id",
                    JsonValue::String(snapshot.index.rule_set_id.clone()),
                ),
                (
                    "serialization_contract_sha256",
                    JsonValue::String(snapshot.serialization_contract_sha256.clone()),
                ),
            ])
        })
        .collect::<Vec<_>>();

    let files = generated_files
        .iter()
        .map(|(path, bytes)| {
            object([
                ("path", JsonValue::String(path.clone())),
                ("sha256", JsonValue::String(sha256_hex(bytes))),
            ])
        })
        .collect::<Vec<_>>();

    let manifest = object([
        (
            "canonicalization",
            JsonValue::String(CANONICALIZATION_ID.to_owned()),
        ),
        ("format", JsonValue::String(MANIFEST_FORMAT.to_owned())),
        ("generated_files", JsonValue::Array(files)),
        (
            "generated_output_sha256",
            JsonValue::String(output_digest.to_owned()),
        ),
        (
            "generator",
            object([
                ("name", JsonValue::String(env!("CARGO_PKG_NAME").to_owned())),
                (
                    "source_sha256",
                    JsonValue::String(generator_digest.to_owned()),
                ),
                (
                    "version",
                    JsonValue::String(env!("CARGO_PKG_VERSION").to_owned()),
                ),
            ]),
        ),
        (
            "normalized_source_sha256",
            JsonValue::String(audit.normalized_source_digest.clone()),
        ),
        (
            "schema_sha256",
            JsonValue::String(audit.schema_digest.clone()),
        ),
        ("snapshots", JsonValue::Array(snapshots)),
    ]);
    let mut bytes = canonical_bytes(&manifest);
    bytes.push(b'\n');
    bytes
}

fn object<const N: usize>(entries: [(&str, JsonValue); N]) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn generated_banner() -> String {
    "// @generated by bir-rules-codegen. DO NOT EDIT.\n\
     // The canonical source is rules/ir/v2; candidates compile only under cfg(test).\n\n"
        .to_owned()
}

fn review_status_label(status: &ReviewStatus) -> &'static str {
    match status {
        ReviewStatus::Skeleton => "skeleton",
        ReviewStatus::Candidate => "candidate",
        ReviewStatus::Reviewed => "reviewed",
    }
}

fn module_name(snapshot: &AuditedSnapshot) -> String {
    format!(
        "form_{}_v{}_p{}",
        identifier_fragment(&snapshot.document.identity.form_code.to_ascii_lowercase()),
        identifier_fragment(&snapshot.document.identity.form_revision),
        identifier_fragment(&snapshot.document.identity.official_package_version),
    )
}

fn identifier_fragment(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        } else if !output.ends_with('_') {
            output.push('_');
        }
    }
    while output.ends_with('_') {
        output.pop();
    }
    output
}

fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

fn rust_raw_string(value: &str) -> String {
    for hashes in 0..=255 {
        let delimiter = "#".repeat(hashes);
        let terminator = format!("\"{delimiter}");
        if !value.contains(&terminator) {
            return format!("r{delimiter}\"{value}\"{delimiter}");
        }
    }
    unreachable!("a JSON document cannot exhaust all Rust raw-string delimiters")
}

fn digest_file_tree(domain: &str, files: &BTreeMap<String, Vec<u8>>) -> String {
    digest_entries(
        domain,
        files
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    )
}

/// Every file whose bytes define the generator's behavior.
///
/// `include_bytes!` cannot be derived from a directory listing, so this list is
/// hand-maintained. `generator_source_list_covers_every_source_file` fails when
/// it falls behind, because a source file outside this list would change what
/// the generator emits while leaving `generator.source_sha256` untouched.
const GENERATOR_SOURCES: &[(&str, &[u8])] = &[
    ("Cargo.toml", include_bytes!("../Cargo.toml")),
    ("src/audit.rs", include_bytes!("audit.rs")),
    ("src/bindings.rs", include_bytes!("bindings.rs")),
    (
        "src/capture_metadata.rs",
        include_bytes!("capture_metadata.rs"),
    ),
    (
        "src/canonicalize_json.rs",
        include_bytes!("canonicalize_json.rs"),
    ),
    ("src/check.rs", include_bytes!("check.rs")),
    ("src/corpus.rs", include_bytes!("corpus.rs")),
    ("src/coverage.rs", include_bytes!("coverage.rs")),
    ("src/emit.rs", include_bytes!("emit.rs")),
    ("src/error.rs", include_bytes!("error.rs")),
    ("src/evidence.rs", include_bytes!("evidence.rs")),
    (
        "src/evidence_review_scaffold.rs",
        include_bytes!("evidence_review_scaffold.rs"),
    ),
    ("src/evidence_set.rs", include_bytes!("evidence_set.rs")),
    ("src/files.rs", include_bytes!("files.rs")),
    ("src/form_factory.rs", include_bytes!("form_factory.rs")),
    (
        "src/form_integration.rs",
        include_bytes!("form_integration.rs"),
    ),
    ("src/generate.rs", include_bytes!("generate.rs")),
    ("src/hash.rs", include_bytes!("hash.rs")),
    ("src/json.rs", include_bytes!("json.rs")),
    ("src/lib.rs", include_bytes!("lib.rs")),
    ("src/main.rs", include_bytes!("main.rs")),
    ("src/model.rs", include_bytes!("model.rs")),
    (
        "src/operator_census.rs",
        include_bytes!("operator_census.rs"),
    ),
    ("src/path.rs", include_bytes!("path.rs")),
    ("src/projections.rs", include_bytes!("projections.rs")),
    ("src/reconciliation.rs", include_bytes!("reconciliation.rs")),
    ("src/rollpin.rs", include_bytes!("rollpin.rs")),
    ("src/schema.rs", include_bytes!("schema.rs")),
    ("src/sensitive.rs", include_bytes!("sensitive.rs")),
    ("src/status.rs", include_bytes!("status.rs")),
    (
        "src/vault_acquisition.rs",
        include_bytes!("vault_acquisition.rs"),
    ),
    (
        "src/vault_source_discovery.rs",
        include_bytes!("vault_source_discovery.rs"),
    ),
    ("src/verified_file.rs", include_bytes!("verified_file.rs")),
];

fn generator_source_digest() -> String {
    digest_entries(
        "bir-rules-codegen-source-v1",
        GENERATOR_SOURCES.iter().copied(),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{GenerateOptions, audit_for_generation, build_generated_files};
    use crate::audit::{AuditOptions, AuditReport, AuditedSnapshot, audit};
    use crate::json::{JsonValue, canonical_bytes};
    use crate::model::{
        BranchState, EvaluationPolicyBranch, IndexSnapshot, ProfileStates, ProfileStatusBranch,
        ReviewStatus, RuleSetDocument, SourceRef,
    };
    use serde_json::json;

    const LANDED_RULE_SET_ID: &str = "2550q-v2024-p7.9.6.0";

    #[test]
    fn required_rule_set_focus_is_a_byte_neutral_presence_assertion() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let baseline_options = GenerateOptions::external_workspace(&root);
        let baseline_audit =
            audit_for_generation(&baseline_options).expect("audit aggregate without focus");
        let baseline =
            build_generated_files(&baseline_audit).expect("generate aggregate without focus");

        let mut focused_options = GenerateOptions::external_workspace(&root);
        focused_options.required_rule_set_id = Some(LANDED_RULE_SET_ID.to_owned());
        let focused_audit =
            audit_for_generation(&focused_options).expect("audit aggregate with focus");
        let focused = build_generated_files(&focused_audit).expect("generate aggregate with focus");

        assert_eq!(
            focused_audit.snapshot_count(),
            baseline_audit.snapshot_count(),
            "focus must not narrow the audit"
        );
        assert_eq!(baseline.files, focused.files);
        assert_eq!(
            baseline.generated_output_digest,
            focused.generated_output_digest
        );
        assert_eq!(baseline.manifest_digest, focused.manifest_digest);
    }

    #[test]
    fn required_rule_set_focus_does_not_hide_unrelated_aggregate_corruption() {
        let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bir-rules-focused-full-audit-{}-{nonce}",
            std::process::id()
        ));
        copy_tree(&source_root.join("rules"), &root.join("rules"));
        fs::write(root.join("rules/ir/v2/unindexed-corruption.json"), b"{}")
            .expect("write unrelated unindexed JSON corruption");

        let mut options = GenerateOptions::external_workspace(&root);
        options.required_rule_set_id = Some(LANDED_RULE_SET_ID.to_owned());
        let error = audit_for_generation(&options)
            .expect_err("full aggregate audit must reject unrelated corruption");
        assert!(
            error.message().contains("fixture file/list bijection"),
            "unexpected error: {error}"
        );

        fs::remove_dir_all(&root).expect("remove focused full-audit test root");
    }

    #[test]
    fn landed_candidate_is_test_only_and_generates_an_empty_reviewed_registry() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let audit = audit(&AuditOptions::external_workspace(root)).expect("audit landed v2 corpus");
        let first = build_generated_files(&audit).expect("generate once");
        let second = build_generated_files(&audit).expect("generate twice");
        assert_eq!(first.files, second.files);
        assert_eq!(first.reviewed_snapshot_count, 0);
        assert_eq!(first.candidate_snapshot_count, 1);
        let module_stem = "form_2550q_v2024_04_01_p7_9_6_0";
        let mod_rs = std::str::from_utf8(&first.files["mod.rs"]).expect("mod.rs is UTF-8");
        assert!(mod_rs.contains(&format!("#[cfg(test)]\nmod {module_stem};")));
        assert!(first.files.contains_key(&format!("{module_stem}.rs")));
        let candidate_module = std::str::from_utf8(&first.files[&format!("{module_stem}.rs")])
            .expect("candidate module is UTF-8");
        assert!(candidate_module.contains("transition_id: \"final-copy-open-enrollment\""));
        assert!(candidate_module.contains("evaluation_phase: ValidationPhase::Validate"));
        assert!(candidate_module.contains("to_state: \"submission-enrollment\""));
        assert!(candidate_module.contains("transition_id: \"submit-after-enrollment\""));
        assert!(candidate_module.contains("evaluation_phase: ValidationPhase::Save"));
        assert!(candidate_module.contains("to_state: \"submission-attempted\""));
        let serialization_contract = candidate_module
            .split("pub static STATIC_SERIALIZATION_CONTRACT")
            .nth(1)
            .expect("candidate serialization contract")
            .split("pub static STATIC_RULE_SET_SPEC")
            .next()
            .expect("candidate serialization contract boundary");
        assert!(serialization_contract.contains("artifact_id: \"official-encrypted-final-copy\""));
        assert!(serialization_contract.contains("artifact_id: \"official-editable-save\""));
        assert!(serialization_contract.contains("artifact_id: \"official-finalized-save\""));
        assert!(
            serialization_contract.contains("target: SerializationArtifactTarget::EditableSave")
        );
        assert!(
            serialization_contract.contains("target: SerializationArtifactTarget::FinalizedSave")
        );
        assert!(
            serialization_contract
                .contains("target: SerializationArtifactTarget::EncryptedFinalCopy")
        );
        assert_eq!(
            serialization_contract
                .matches("variant_id: \"p7.9.6.0-dom-order\"")
                .count(),
            3
        );
        assert_eq!(
            serialization_contract
                .matches("official: Branch::DocumentedOnly")
                .count(),
            3
        );
        assert_eq!(
            serialization_contract
                .matches("filing_safe: Branch::Unresolved")
                .count(),
            3
        );
        assert!(!serialization_contract.contains("nodes:"));
        assert!(
            candidate_module
                .matches("official: Branch::DocumentedOnly")
                .count()
                >= 2
        );
        let registry = std::str::from_utf8(&first.files["registry.rs"]).expect("registry is UTF-8");
        assert!(registry.contains("REVIEWED_RULE_SET_METADATA"));
        assert!(
            registry.contains("REVIEWED_RULE_SET_METADATA: &[GeneratedRuleSetMetadata] = &[];")
        );
        assert!(registry.contains("reviewed_rule_set_entries"));
        assert!(registry.contains(
            "static REVIEWED_RULE_SET_ENTRIES: LazyLock<Vec<RuleSetRegistryEntry>> = \
             LazyLock::new(|| vec![]);"
        ));
        assert!(registry.contains("#[cfg(test)]\npub static CANDIDATE_RULE_SET_METADATA"));
        assert!(registry.contains("#[cfg(test)]\nstatic CANDIDATE_RULE_SET_ENTRIES"));
        assert!(registry.contains("#[cfg(test)]\npub fn candidate_rule_set_entries"));
        assert!(registry.contains(module_stem));
        assert!(!registry.contains("\"2550q-v2024-p7.9.6.0\""));
    }

    #[test]
    fn landed_2550q_serialization_binding_inventory_is_value_free_and_complete() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path =
            root.join("rules/forms/2550q-v2024/fixtures/serialization-binding-inventory-v796.json");
        let bytes = fs::read(&path).expect("read 2550Q serialization binding inventory");
        let inventory: serde_json::Value =
            serde_json::from_slice(&bytes).expect("parse 2550Q serialization binding inventory");

        assert_eq!(inventory["status"], "documented-only");
        assert_eq!(inventory["values_emitted"], false);
        assert_eq!(
            inventory["observed_contract"]["plaintext_occurrence_count"],
            160
        );
        assert_eq!(
            inventory["observed_contract"]["encrypted_pseudo_div_occurrence_count"],
            159
        );
        assert_eq!(
            inventory["observed_contract"]["candidate_v2_bound_occurrence_count"],
            160
        );
        assert_eq!(
            inventory["observed_contract"]["encrypted_is_exact_plaintext_prefix"],
            true
        );
        assert_eq!(
            inventory["observed_contract"]["plaintext_only_suffix"],
            "dateFiled"
        );

        let occurrences = inventory["occurrence_bindings"]
            .as_array()
            .expect("occurrence bindings array");
        assert_eq!(occurrences.len(), 160);
        assert_eq!(occurrences[0]["plaintext_ordinal"], 1);
        assert_eq!(occurrences[0]["key"], "frm2550qv2024:calendarNo1");
        assert_eq!(occurrences[159]["plaintext_ordinal"], 160);
        assert_eq!(
            occurrences[159]["encrypted_ordinal"],
            serde_json::Value::Null
        );
        assert_eq!(occurrences[159]["key"], "dateFiled");
        assert_eq!(
            occurrences[159]["source_projection"]["v2_context_value_id"],
            "local-current-date"
        );
        assert_eq!(
            occurrences[159]["plaintext_save"]["placement"],
            "pseudo-div"
        );
        assert_eq!(
            occurrences[159]["encrypted_staging"]["placement"],
            "standalone-metadata-after-marker"
        );

        for key in [
            "frm2550qv2024:taxpayerName",
            "frm2550qv2024:taxpayerAddress",
        ] {
            let binding = occurrences
                .iter()
                .find(|binding| binding["key"] == key)
                .expect("escaped plaintext binding");
            assert_eq!(
                binding["plaintext_save"]["body_codec"],
                "legacy-javascript-escape"
            );
            assert_eq!(binding["encrypted_staging"]["body_codec"], "raw-literal");
        }

        let groups = inventory["dynamic_groups"]
            .as_array()
            .expect("dynamic groups array");
        assert_eq!(groups.len(), 7);
        assert_eq!(
            groups
                .iter()
                .map(|group| group["group_id"].as_str().expect("group ID"))
                .collect::<Vec<_>>(),
            vec![
                "schedule-1-capital-good-row",
                "schedule-3-creditable-vat-row",
                "schedule-4-advance-vat-row",
                "item-19-additional-row",
                "item-42-additional-row",
                "item-47-additional-row",
                "item-56-additional-row",
            ]
        );
        assert_eq!(
            groups
                .iter()
                .map(|group| group["families"].as_array().expect("group families").len())
                .sum::<usize>(),
            28
        );
        assert!(groups.iter().all(|group| group["max_occurs"].is_null()));
        assert!(
            groups
                .iter()
                .all(|group| group["instance_identity"] == "assigned-stable-id")
        );
        assert!(
            groups
                .iter()
                .all(|group| group["serialization_instance_order_relation"] == "unresolved")
        );

        let text = std::str::from_utf8(&bytes).expect("inventory is UTF-8");
        assert!(!text.contains("\"value\":"));

        let rule_set_path = root.join("rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json");
        let rule_set_bytes = fs::read(rule_set_path).expect("read 2550Q candidate rule set");
        let rule_set: serde_json::Value =
            serde_json::from_slice(&rule_set_bytes).expect("parse 2550Q candidate rule set");
        let candidate_groups = rule_set["field_groups"]
            .as_array()
            .expect("candidate field groups");
        let candidate_fields = rule_set["fields"].as_array().expect("candidate fields");
        assert_eq!(candidate_groups.len(), 7);
        assert_eq!(candidate_fields.len(), 94);
        assert_eq!(
            candidate_groups
                .iter()
                .map(|group| group["members"].as_array().expect("group members").len())
                .sum::<usize>(),
            28
        );
        assert!(candidate_groups.iter().all(|group| {
            group["min_occurs"] == 0
                && group["max_occurs"].is_null()
                && group["instance_identity"] == "assigned-stable-id"
        }));

        let grouped_fields = candidate_fields
            .iter()
            .filter(|field| !field["group_id"].is_null())
            .collect::<Vec<_>>();
        assert_eq!(grouped_fields.len(), 28);
        assert!(grouped_fields.iter().all(|field| {
            field["value_type"] == "string"
                && field["control_kind"] == "text"
                && field["requiredness"] == "conditional"
                && field["serialized"]
                    .as_array()
                    .is_some_and(|projections| projections.is_empty())
                && field["behavior"]["official"]["state"] == "executable"
                && field["behavior"]["official"]["normalization"]
                    .as_array()
                    .is_some_and(|normalization| normalization.is_empty())
                && field["behavior"]["official"]["coercion"]["kind"] == "string"
                && field["behavior"]["official"]["coercion"]["on_empty"] == "empty-string"
                && field["behavior"]["filing_safe"]["state"] == "unresolved"
        }));

        let static_surface_fields = candidate_fields
            .iter()
            .filter(|field| {
                field["source_refs"].as_array().is_some_and(|source_refs| {
                    source_refs.iter().any(|source_ref| {
                        source_ref["source_id"] == "candidate-static-surface-projection-review"
                    })
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(static_surface_fields.len(), 34);
        assert!(static_surface_fields.iter().all(|field| {
            field["group_id"].is_null()
                && field["serialized"]
                    .as_array()
                    .is_some_and(|projections| projections.is_empty())
                && field["behavior"]["official"]["state"] == "executable"
                && field["behavior"]["filing_safe"]["state"] == "unresolved"
        }));
        for sensitive_field in [
            "ebirOnlineUsername",
            "ebirOnlineConfirmUsername",
            "ebirOnlineSecret",
        ] {
            assert!(
                candidate_fields
                    .iter()
                    .all(|field| field["field_id"] != sensitive_field),
                "workflow credential must remain outside the executable field surface"
            );
            assert!(
                occurrences
                    .iter()
                    .any(|binding| binding["candidate_v2_field_id"] == sensitive_field),
                "workflow credential identity remains value-free and source-bound"
            );
        }

        let artifacts = rule_set["serialization"]["artifacts"]
            .as_array()
            .expect("serialization artifacts");
        assert_eq!(artifacts.len(), 3);
        assert!(artifacts.iter().all(|artifact| {
            artifact["official"]["state"] == "documented_only"
                && artifact.get("nodes").map_or(true, |nodes| {
                    nodes.as_array().is_some_and(|nodes| nodes.is_empty())
                })
        }));
    }

    #[test]
    fn synthetic_reviewed_snapshot_emits_deterministic_executable_rust() {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_reviewed_snapshot(
            "test-v1-p1",
            "TEST",
            "emit-issue",
        )];

        let first = build_generated_files(&report).expect("emit executable snapshot once");
        let second = build_generated_files(&report).expect("emit executable snapshot twice");
        assert_eq!(first.files, second.files);
        assert_eq!(first.reviewed_snapshot_count, 1);

        let module_name = "form_test_v2024_01_01_p1_0_0.rs";
        let module = std::str::from_utf8(&first.files[module_name]).expect("module is UTF-8");
        assert!(module.contains("pub static STATIC_RULE_SET_SPEC: StaticRuleSetSpec"));
        assert!(module.contains("EffectEvaluationMode::StopEffectsAfterFirstBlockingIssue"));
        assert!(module.contains("EffectEvaluationMode::ApplyAll"));
        assert!(module.contains("Expression::Field"));
        assert!(module.contains("scope: EvaluationScope::Singleton"));
        assert!(module.contains("Predicate::Compare"));
        assert!(module.contains("Effect::EmitIssue"));
        assert!(module.contains("pub static STATIC_SERIALIZATION_CONTRACT"));
        assert!(module.contains("canonical_sha256: Some("));
        assert!(module.contains("SerializationArtifactTarget::EditableSave"));
        assert!(module.contains("SerializationNode::PseudoXmlField"));
        assert!(module.contains("SerializationNode::MetadataElement"));
        assert!(module.contains("SerializationNode::ReviewedLiteral"));
        assert!(module.contains("SerializationNode::DynamicGroup"));
        assert!(module.contains("SerializationValueProjection::Default"));
        assert!(module.contains("SerializationPresence::When(Predicate::Constant(true))"));
        assert!(module.contains("BodyCodec::LegacyJavaScriptEscape"));
        assert!(module.contains("SerializationDecimalFormat"));
        assert!(!module.contains("ExactDecimalFormat"));
        assert!(module.contains("StaticCompiledRuleSet::new("));
        assert!(module.contains("LazyLock<StaticCompiledRuleSet>"));
        assert!(module.contains("serde_json::from_str(fixture.input_json)"));
        assert!(module.contains("serde_json::from_str(fixture.expected_json)"));
        assert!(module.contains("CompiledRuleSet::evaluate(&*COMPILED_RULE_SET, &request)"));
        assert!(module.contains("assert_eq!("));

        let registry = std::str::from_utf8(&first.files["registry.rs"]).unwrap();
        assert!(registry.contains("RuleSetRegistryEntry::new("));
        assert!(registry.contains("&*super::form_test_v2024_01_01_p1_0_0::COMPILED_RULE_SET"));
    }

    #[test]
    fn synthetic_reviewed_snapshot_embeds_complete_fixture_values_in_runtime_test() {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_reviewed_snapshot(
            "test-v1-p1",
            "TEST",
            "emit-issue",
        )];

        let generation =
            build_generated_files(&report).expect("emit reviewed fixture execution test");
        let module =
            std::str::from_utf8(&generation.files["form_test_v2024_01_01_p1_0_0.rs"]).unwrap();

        assert!(module.contains("#[cfg(test)]"));
        assert!(module.contains("evaluation_fixtures_match_compiled_provider"));
        assert!(module.contains("sentinel-row-007"));
        assert!(module.contains("SENTINEL fixture canonical input value"));
        assert!(module.contains("SENTINEL fixture violation message."));
        assert!(module.contains("\"canonical_inputs\""));
        assert!(module.contains("\"derived_outputs\""));
        assert!(module.contains("\"violations\""));
    }

    #[test]
    fn candidate_is_cfg_test_only_fixture_executed_and_excluded_from_reviewed_registry() {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_candidate_snapshot()];

        let first = build_generated_files(&report).expect("emit candidate once");
        let second = build_generated_files(&report).expect("emit candidate twice");
        assert_eq!(first.files, second.files);
        assert_eq!(first.reviewed_snapshot_count, 0);
        assert_eq!(first.candidate_snapshot_count, 1);

        let module_stem = "form_test_v2024_01_01_p1_0_0";
        let module_name = format!("{module_stem}.rs");
        let mod_rs = std::str::from_utf8(&first.files["mod.rs"]).unwrap();
        assert!(mod_rs.contains(&format!("#[cfg(test)]\nmod {module_stem};")));
        let module_line = format!("mod {module_stem};");
        let lines = mod_rs.lines().collect::<Vec<_>>();
        let module_index = lines
            .iter()
            .position(|line| *line == module_line.as_str())
            .expect("candidate module declaration");
        assert_eq!(lines[module_index - 1], "#[cfg(test)]");

        let module = std::str::from_utf8(first.files.get(&module_name).unwrap()).unwrap();
        assert!(module.contains("Branch::Unresolved"));
        assert!(module.contains("Branch::DocumentedOnly"));
        assert!(module.contains("evaluation_fixtures_match_compiled_provider"));
        assert!(module.contains("CompiledRuleSet::evaluate(&*COMPILED_RULE_SET, &request)"));
        assert!(module.contains("assert_eq!("));
        assert!(module.contains("SENTINEL fixture violation message."));

        let registry = std::str::from_utf8(&first.files["registry.rs"]).unwrap();
        assert!(
            registry.contains("REVIEWED_RULE_SET_METADATA: &[GeneratedRuleSetMetadata] = &[];")
        );
        let (production_catalog, candidate_catalog) = registry
            .split_once("// Candidate providers exist for library verification only.")
            .expect("candidate registry must have a clearly delimited test-only catalog");
        assert!(!production_catalog.contains(module_stem));
        assert!(!production_catalog.contains("test-v1-p1"));
        assert!(candidate_catalog.contains("#[cfg(test)]\npub static CANDIDATE_RULE_SET_METADATA"));
        assert!(candidate_catalog.contains("#[cfg(test)]\nstatic CANDIDATE_RULE_SET_ENTRIES"));
        assert!(candidate_catalog.contains("#[cfg(test)]\npub fn candidate_rule_set_entries"));
        assert!(candidate_catalog.contains(module_stem));

        let manifest = std::str::from_utf8(&first.files["manifest.json"]).unwrap();
        assert!(manifest.contains("\"review_status\":\"candidate\""));
        assert!(manifest.contains(&format!("\"generated_module\":\"{module_stem}\"")));
        assert!(manifest.contains(&format!("\"path\":\"{module_name}\"")));
        assert!(manifest.contains("\"sha256\":"));
    }

    #[test]
    fn candidate_generated_module_compiles_and_executes_its_fixture() {
        let report = synthetic_audit(vec![synthetic_candidate_snapshot()]);
        let generation = build_generated_files(&report).expect("emit candidate probe");

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bir-rules-candidate-probe-{}-{nonce}",
            std::process::id()
        ));
        let crate_root = root.join("crates/bir-rules");
        let generated = crate_root.join("src/generated");
        let source_crate = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../bir-rules");
        copy_tree(&source_crate.join("src"), &crate_root.join("src"));
        fs::copy(
            source_crate.join("Cargo.toml"),
            crate_root.join("Cargo.toml"),
        )
        .expect("copy bir-rules manifest");
        fs::remove_dir_all(&generated).expect("remove landed generated source from probe");
        fs::create_dir_all(&generated).expect("create candidate probe source tree");
        for (path, bytes) in &generation.files {
            if path.ends_with(".rs") {
                fs::write(generated.join(path), bytes).expect("write generated candidate probe");
            }
        }

        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\n\
             members = [\"crates/bir-rules\"]\n\
             resolver = \"2\"\n\n\
             [workspace.package]\n\
             version = \"0.0.0\"\n\
             edition = \"2024\"\n\
             license = \"MIT\"\n\n\
             [workspace.dependencies]\n\
             serde = { version = \"1\", features = [\"derive\"] }\n\
             serde_json = \"1\"\n\
             sha2 = \"0.10\"\n",
        )
        .expect("write candidate probe workspace");

        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let isolated_target = root.join("target");
        let output = Command::new(cargo)
            .current_dir(&root)
            .args(["test", "--offline", "-p", "bir-rules"])
            // The parent test process is itself running under Cargo. Use an
            // explicit, probe-local target so the nested build can never wait
            // on the parent's target-directory lock.
            .env_remove("CARGO_TARGET_DIR")
            .arg("--target-dir")
            .arg(&isolated_target)
            .output()
            .expect("run generated candidate probe tests");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "generated candidate probe failed in `{}`\nstdout:\n{stdout}\nstderr:\n{stderr}",
            root.display()
        );
        assert!(
            stdout.contains("evaluation_fixtures_match_compiled_provider"),
            "generated candidate fixture test did not execute\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );

        let mut cleanup_error = None;
        for attempt in 0..100 {
            match fs::remove_dir_all(&root) {
                Ok(()) => {
                    cleanup_error = None;
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    cleanup_error = None;
                    break;
                }
                Err(error) => {
                    cleanup_error = Some(error);
                    if attempt < 99 {
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        }
        if let Some(error) = cleanup_error {
            panic!(
                "remove successful candidate probe `{}` after retries: {error}",
                root.display()
            );
        }
    }

    #[test]
    fn incomplete_candidate_fails_closed() {
        let mut report = landed_audit();
        let mut no_fixtures = synthetic_candidate_snapshot();
        no_fixtures.fixtures.clear();
        report.snapshots = vec![no_fixtures];
        let error = build_generated_files(&report)
            .expect_err("candidate must include an executable evaluation fixture");
        assert!(
            error
                .message()
                .contains("must have at least one concrete evaluation fixture")
        );

        let mut no_digest = synthetic_candidate_snapshot();
        no_digest.document.identity.source_set_sha256 = None;
        no_digest.index.source_set_sha256 = None;
        report.snapshots = vec![no_digest];
        let error = build_generated_files(&report)
            .expect_err("candidate must pin normalized source digest");
        assert!(error.message().contains("must pin source_set_sha256"));

        let mut no_executable_pair = synthetic_candidate_snapshot();
        no_executable_pair.document.profile_status.official = ProfileStatusBranch::DocumentedOnly {
            summary: "not executable".to_owned(),
            source_refs: vec![source_ref()],
        };
        no_executable_pair.index.profile_states.official = BranchState::DocumentedOnly;
        report.snapshots = vec![no_executable_pair];
        let error = build_generated_files(&report)
            .expect_err("candidate must preserve an executable profile/policy pair");
        assert!(error.message().contains(
            "at least one profile whose rule-set and evaluation-policy branches are both executable"
        ));
    }

    #[test]
    fn candidate_rejects_non_evaluation_fixture_and_non_executable_fixture_profile() {
        let mut report = landed_audit();
        let mut compile_rejection = synthetic_candidate_snapshot();
        compile_rejection
            .fixtures
            .values_mut()
            .next()
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "kind".to_owned(),
                JsonValue::String("compile-rejection".to_owned()),
            );
        report.snapshots = vec![compile_rejection];
        let error = build_generated_files(&report)
            .expect_err("candidate must reject non-evaluation fixtures");
        assert!(
            error
                .message()
                .contains("non-evaluation kind `compile-rejection`")
        );

        let mut fallback_attempt = synthetic_candidate_snapshot();
        let fixture = fallback_attempt.fixtures.values_mut().next().unwrap();
        fixture
            .object_mut()
            .unwrap()
            .get_mut("input")
            .unwrap()
            .object_mut()
            .unwrap()
            .get_mut("context")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "profile".to_owned(),
                JsonValue::String("filing_safe".to_owned()),
            );
        report.snapshots = vec![fallback_attempt];
        let error = build_generated_files(&report)
            .expect_err("candidate fixture must not fall back from non-executable branch");
        assert!(error.message().contains("branches never fall back"));
    }

    #[test]
    fn candidate_executable_nodes_use_the_same_fail_closed_emitter_as_reviewed_nodes() {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_candidate_snapshot_with_effect("set-derived")];
        let error = build_generated_files(&report)
            .expect_err("unsupported executable candidate node must fail generation");
        assert!(error.message().contains("unsupported executable node"));
        assert!(error.message().contains("set-derived"));
        assert!(
            error
                .message()
                .contains("dependent generated outputs internally inconsistent")
        );
    }

    #[test]
    fn reviewed_snapshot_without_evaluation_fixtures_fails_generation() {
        let mut report = landed_audit();
        let mut snapshot = synthetic_reviewed_snapshot("test-v1-p1", "TEST", "emit-issue");
        snapshot.fixtures.clear();
        report.snapshots = vec![snapshot];

        let error = build_generated_files(&report)
            .expect_err("reviewed emission must require concrete evaluation fixtures");
        assert!(error.message().contains("test-v1-p1"));
        assert!(
            error
                .message()
                .contains("without concrete evaluation fixtures")
        );
    }

    #[test]
    fn reviewed_snapshot_with_non_evaluation_fixture_fails_generation() {
        let mut report = landed_audit();
        let mut snapshot = synthetic_reviewed_snapshot("test-v1-p1", "TEST", "emit-issue");
        snapshot
            .fixtures
            .values_mut()
            .next()
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "kind".to_owned(),
                JsonValue::String("compile-rejection".to_owned()),
            );
        report.snapshots = vec![snapshot];

        let error = build_generated_files(&report)
            .expect_err("reviewed emission must reject non-evaluation fixtures");
        assert!(error.message().contains("synthetic-reviewed-evaluation"));
        assert!(
            error
                .message()
                .contains("non-evaluation kind `compile-rejection`")
        );
    }

    #[test]
    fn coercion_failed_predicate_emits_deterministically() {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_reviewed_snapshot(
            "test-v1-p1",
            "TEST",
            "coercion-failed",
        )];

        let first = build_generated_files(&report).expect("emit coercion-failed predicate");
        let second = build_generated_files(&report).expect("repeat deterministic generation");
        assert_eq!(first.files, second.files);

        let module = std::str::from_utf8(&first.files["form_test_v2024_01_01_p1_0_0.rs"])
            .expect("generated module is UTF-8");
        assert!(module.contains("Predicate::CoercionFailed {"));
        assert!(module.contains("field: FieldRef {"));
        assert!(module.contains("field_id: \"amount\""));
        assert!(module.contains("instance: FieldInstanceSelector::Singleton"));
    }

    #[test]
    fn generation_rejects_overlapping_boolean_coercion_tokens() {
        let mut report = landed_audit();
        let mut snapshot = synthetic_reviewed_snapshot("test-v1-p1", "TEST", "emit-issue");
        let field = snapshot.document.fields[0]
            .object_mut()
            .expect("synthetic field is an object");
        let behavior = field
            .get_mut("behavior")
            .and_then(JsonValue::object_mut)
            .expect("synthetic behavior is an object");
        for profile in ["official", "filing_safe"] {
            let branch = behavior
                .get_mut(profile)
                .and_then(JsonValue::object_mut)
                .expect("synthetic behavior branch is an object");
            branch.insert(
                "coercion".to_owned(),
                serde_json::from_value(json!({
                    "kind": "boolean",
                    "true_values": ["Y", "1"],
                    "false_values": ["N", "Y"],
                    "on_empty": "null",
                    "on_invalid": "error"
                }))
                .expect("build overlapping boolean coercion"),
            );
        }
        report.snapshots = vec![snapshot];

        let error = build_generated_files(&report)
            .expect_err("generation must reject an ambiguous boolean coercion");
        assert!(
            error
                .message()
                .contains("maps \"Y\" as both true and false")
        );
        assert!(error.message().contains("$.fields[0].behavior.official"));
    }

    #[test]
    fn reviewed_snapshot_without_both_executable_policies_fails_closed() {
        let mut report = landed_audit();
        let mut snapshot = synthetic_reviewed_snapshot("test-v1-p1", "TEST", "emit-issue");
        snapshot.document.evaluation_policy.filing_safe = EvaluationPolicyBranch::Unresolved {
            reason: "not independently reviewed".to_owned(),
            source_refs: Vec::new(),
        };
        report.snapshots = vec![snapshot];

        let error =
            build_generated_files(&report).expect_err("policy must never default across profiles");
        assert!(error.message().contains("test-v1-p1"));
        assert!(error.message().contains("evaluation-policy"));
        assert!(error.message().contains("filing_safe"));
        assert!(error.message().contains("unresolved"));
    }

    #[test]
    fn unsupported_workflow_state_effect_is_an_explicit_generation_error() {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_reviewed_snapshot(
            "test-v1-p1",
            "TEST",
            "set-workflow-state",
        )];

        let error =
            build_generated_files(&report).expect_err("unsupported effect must fail generation");
        assert!(error.message().contains("unsupported executable node"));
        assert!(error.message().contains("effect.set-workflow-state"));
        assert!(error.message().contains("EvaluationResult"));
        assert!(error.message().contains("$.rules[0]"));
    }

    #[test]
    fn unsupported_set_derived_effect_is_an_explicit_generation_error() {
        assert_unsupported_effect(
            "set-derived",
            "effect.set-derived",
            "dependent generated outputs",
        );
    }

    #[test]
    fn unsupported_normalize_field_effect_is_an_explicit_generation_error() {
        assert_unsupported_effect(
            "normalize-field",
            "effect.normalize-field",
            "dependent generated outputs",
        );
    }

    #[test]
    fn decimal_division_without_expression_policy_fails_generation() {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_reviewed_snapshot(
            "test-v1-p1",
            "TEST",
            "decimal-divide",
        )];

        let error = build_generated_files(&report)
            .expect_err("decimal division without an expression policy must fail generation");
        assert!(error.message().contains("division_policy"));
        assert!(error.message().contains("missing required property"));
        assert!(error.message().contains("$.rules[0]"));
    }

    #[test]
    fn policy_bound_decimal_division_emits_deterministically() {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_reviewed_snapshot(
            "test-v1-p1",
            "TEST",
            "decimal-divide-policy",
        )];

        let first = build_generated_files(&report).expect("emit policy-bound decimal division");
        let second = build_generated_files(&report).expect("repeat deterministic generation");
        assert_eq!(first.files, second.files);
        let module = std::str::from_utf8(&first.files["form_test_v2024_01_01_p1_0_0.rs"])
            .expect("generated module is UTF-8");
        assert!(module.contains("division_policy: Some(DecimalDivisionPolicy {"));
        assert!(module.contains("scale: 2"));
        assert!(module.contains("rounding: RoundingMode::HalfEven"));
    }

    #[test]
    fn decimal_division_policy_on_non_divide_fails_generation() {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_reviewed_snapshot(
            "test-v1-p1",
            "TEST",
            "add-with-division-policy",
        )];

        let error = build_generated_files(&report)
            .expect_err("non-divide operator must reject decimal division policy");
        assert!(error.message().contains("operator `add`"));
        assert!(error.message().contains("must not carry `division_policy`"));
        assert!(error.message().contains("$.rules[0]"));
    }

    #[test]
    fn decimal_division_policy_scale_above_limit_fails_generation() {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_reviewed_snapshot(
            "test-v1-p1",
            "TEST",
            "decimal-divide-invalid-scale",
        )];

        let error = build_generated_files(&report)
            .expect_err("invalid expression division scale must fail generation");
        assert!(
            error
                .message()
                .contains("decimal division scale 19 exceeds 18")
        );
        assert!(error.message().contains("$.rules[0]"));
    }

    #[test]
    fn decimal_scale_exceeding_precision_fails_generation() {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_reviewed_snapshot(
            "test-v1-p1",
            "TEST",
            "scale-exceeds-precision",
        )];

        let error = build_generated_files(&report)
            .expect_err("invalid decimal policy must fail during generation");
        assert!(error.message().contains("invalid decimal policy"));
        assert!(error.message().contains("scale 3 exceeds precision 2"));
        assert!(error.message().contains("$.fields[0]"));
    }

    #[test]
    fn executable_registry_is_sorted_by_complete_identity_not_input_or_module_order() {
        let mut report = landed_audit();
        report.snapshots = vec![
            synthetic_reviewed_snapshot("z-v1-p1", "AAAA", "emit-issue"),
            synthetic_reviewed_snapshot("a-v1-p1", "ZZZZ", "emit-issue"),
        ];

        let generation = build_generated_files(&report).expect("emit sorted registry");
        let registry = std::str::from_utf8(&generation.files["registry.rs"]).unwrap();
        let a_identity_entry = registry
            .find("super::form_zzzz_v2024_01_01_p1_0_0::COMPILED_RULE_SET")
            .unwrap();
        let z_identity_entry = registry
            .find("super::form_aaaa_v2024_01_01_p1_0_0::COMPILED_RULE_SET")
            .unwrap();
        assert!(
            a_identity_entry < z_identity_entry,
            "rule_set_id `a-v1-p1` must sort before `z-v1-p1` even though its module sorts later"
        );
    }

    /// A generator source file outside `GENERATOR_SOURCES` would change what
    /// the generator emits while leaving `generator.source_sha256` unchanged,
    /// silently breaking the provenance the manifest claims to record.
    #[test]
    fn generator_source_list_covers_every_source_file() {
        let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut on_disk: Vec<String> = fs::read_dir(&source_dir)
            .expect("read generator source directory")
            .map(|entry| entry.expect("read generator source entry").path())
            .filter(|path| path.extension() == Some(std::ffi::OsStr::new("rs")))
            .map(|path| {
                format!(
                    "src/{}",
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .expect("generator source name is UTF-8")
                )
            })
            .collect();
        on_disk.sort();

        let mut listed: Vec<String> = super::GENERATOR_SOURCES
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .filter(|name| name.starts_with("src/"))
            .collect();
        listed.sort();

        assert_eq!(
            listed, on_disk,
            "GENERATOR_SOURCES is out of sync with crates/bir-rules-codegen/src"
        );
    }

    fn landed_audit() -> crate::audit::AuditReport {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        audit(&AuditOptions::external_workspace(root)).expect("audit landed v2 corpus")
    }

    fn synthetic_audit(snapshots: Vec<AuditedSnapshot>) -> AuditReport {
        AuditReport {
            repo_root: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .expect("canonical repository root"),
            schema_digest: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_owned(),
            normalized_source_digest:
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned(),
            snapshots,
        }
    }

    fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
        fs::create_dir_all(destination).expect("create copied source directory");
        for entry in fs::read_dir(source).expect("read source directory") {
            let entry = entry.expect("read source entry");
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if entry.file_type().expect("read source entry type").is_dir() {
                copy_tree(&source_path, &destination_path);
            } else {
                fs::copy(&source_path, &destination_path).expect("copy source file");
            }
        }
    }

    fn assert_unsupported_effect(effect_kind: &str, node: &str, reason: &str) {
        let mut report = landed_audit();
        report.snapshots = vec![synthetic_reviewed_snapshot(
            "test-v1-p1",
            "TEST",
            effect_kind,
        )];

        let error =
            build_generated_files(&report).expect_err("unsupported effect must fail generation");
        assert!(error.message().contains("unsupported executable node"));
        assert!(error.message().contains(node));
        assert!(error.message().contains(reason));
        assert!(error.message().contains("$.rules[0]"));
    }

    fn source_ref() -> SourceRef {
        SourceRef {
            source_id: "review".to_owned(),
            locator: None,
        }
    }

    fn synthetic_candidate_snapshot() -> AuditedSnapshot {
        synthetic_candidate_snapshot_with_effect("emit-issue")
    }

    fn synthetic_candidate_snapshot_with_effect(effect_kind: &str) -> AuditedSnapshot {
        let mut snapshot = synthetic_reviewed_snapshot("test-v1-p1", "TEST", effect_kind);
        snapshot.index.review_status = ReviewStatus::Candidate;
        snapshot.document.review_status = ReviewStatus::Candidate;
        snapshot.document.profile_status.filing_safe = ProfileStatusBranch::Unresolved {
            reason: "not yet reviewed".to_owned(),
            source_refs: vec![source_ref()],
        };
        snapshot.index.profile_states.filing_safe = BranchState::Unresolved;
        snapshot.document.evaluation_policy.filing_safe = EvaluationPolicyBranch::DocumentedOnly {
            summary: "not yet reviewed".to_owned(),
            source_refs: vec![source_ref()],
        };

        let mut canonical: serde_json::Value =
            serde_json::from_slice(&snapshot.canonical_rule_set).unwrap();
        canonical
            .as_object_mut()
            .unwrap()
            .insert("review_status".to_owned(), json!("candidate"));
        canonical
            .as_object_mut()
            .unwrap()
            .get_mut("profile_status")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(
                "filing_safe".to_owned(),
                json!({
                    "state": "unresolved",
                    "reason": "not yet reviewed",
                    "source_refs": [{"source_id": "review"}]
                }),
            );
        canonical
            .as_object_mut()
            .unwrap()
            .get_mut("evaluation_policy")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(
                "filing_safe".to_owned(),
                json!({
                    "state": "documented_only",
                    "summary": "not yet reviewed",
                    "source_refs": [{"source_id": "review"}]
                }),
            );
        let canonical: JsonValue = serde_json::from_value(canonical).unwrap();
        snapshot.canonical_rule_set = canonical_bytes(&canonical);
        snapshot
    }

    fn synthetic_reviewed_snapshot(
        rule_set_id: &str,
        form_code: &str,
        effect_kind: &str,
    ) -> AuditedSnapshot {
        const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let effect = match effect_kind {
            "emit-issue"
            | "decimal-divide"
            | "decimal-divide-policy"
            | "decimal-divide-invalid-scale"
            | "add-with-division-policy"
            | "scale-exceeds-precision"
            | "coercion-failed" => json!({
                "kind": "emit-issue",
                "severity": "blocking",
                "message": "SENTINEL fixture violation message.",
                "official_message": null,
                "assessment": "verified-correct",
                "fields": [{
                    "field_id": "amount",
                    "instance": {"kind": "singleton"}
                }]
            }),
            "set-derived" => json!({
                "kind": "set-derived",
                "output_id": "calculated-total",
                "value": {
                    "kind": "literal",
                    "value": {"type": "decimal", "value": "1.00"}
                }
            }),
            "normalize-field" => json!({
                "kind": "normalize-field",
                "field": {
                    "field_id": "amount",
                    "instance": {"kind": "singleton"}
                },
                "normalization": {"kind": "trim", "side": "both"}
            }),
            "set-workflow-state" => json!({
                "kind": "set-workflow-state",
                "state_id": "ready"
            }),
            other => panic!("unknown synthetic effect {other}"),
        };
        let official_predicate = match effect_kind {
            "coercion-failed" => json!({
                "kind": "coercion-failed",
                "field": {
                    "field_id": "amount",
                    "instance": {"kind": "singleton"}
                }
            }),
            "decimal-divide" => json!({
                "kind": "compare",
                "operator": "greater-than-or-equal",
                "left": {
                    "kind": "binary",
                    "result_type": "decimal",
                    "operator": "divide",
                    "left": {
                        "kind": "literal",
                        "value": {"type": "decimal", "value": "1.00"}
                    },
                    "right": {
                        "kind": "literal",
                        "value": {"type": "decimal", "value": "3.00"}
                    }
                },
                "right": {
                    "kind": "literal",
                    "value": {"type": "decimal", "value": "0.00"}
                }
            }),
            "decimal-divide-policy" | "decimal-divide-invalid-scale" => json!({
                "kind": "compare",
                "operator": "greater-than-or-equal",
                "left": {
                    "kind": "binary",
                    "result_type": "decimal",
                    "operator": "divide",
                    "division_policy": {
                        "scale": if effect_kind == "decimal-divide-invalid-scale" { 19 } else { 2 },
                        "rounding": "half-even"
                    },
                    "left": {
                        "kind": "literal",
                        "value": {"type": "decimal", "value": "1.00"}
                    },
                    "right": {
                        "kind": "literal",
                        "value": {"type": "decimal", "value": "3.00"}
                    }
                },
                "right": {
                    "kind": "literal",
                    "value": {"type": "decimal", "value": "0.00"}
                }
            }),
            "add-with-division-policy" => json!({
                "kind": "compare",
                "operator": "greater-than-or-equal",
                "left": {
                    "kind": "binary",
                    "result_type": "decimal",
                    "operator": "add",
                    "division_policy": {
                        "scale": 2,
                        "rounding": "half-up"
                    },
                    "left": {
                        "kind": "literal",
                        "value": {"type": "decimal", "value": "1.00"}
                    },
                    "right": {
                        "kind": "literal",
                        "value": {"type": "decimal", "value": "3.00"}
                    }
                },
                "right": {
                    "kind": "literal",
                    "value": {"type": "decimal", "value": "0.00"}
                }
            }),
            _ => json!({
                "kind": "compare",
                "operator": "less-than",
                "left": {
                    "kind": "field",
                    "result_type": "decimal",
                    "field": {
                        "field_id": "amount",
                        "instance": {"kind": "singleton"}
                    }
                },
                "right": {
                    "kind": "literal",
                    "value": {"type": "decimal", "value": "0.00"}
                }
            }),
        };
        let (decimal_precision, decimal_scale) = if effect_kind == "scale-exceeds-precision" {
            (2, 3)
        } else {
            (12, 2)
        };
        let document_json = json!({
            "$schema": "../../../schema/v2/rule-set.schema.json",
            "schema_version": "2.0.0",
            "identity": {
                "rule_set_id": rule_set_id,
                "form_code": form_code,
                "form_revision": "2024-01-01",
                "official_package_version": "1.0.0",
                "source_set_sha256": DIGEST
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
            "sources": [{
                "source_id": "review",
                "kind": "review-decision",
                "path": "synthetic/review.json",
                "sha256": DIGEST
            }],
            "legacy_v1": {
                "form_id": "test-v1",
                "schema_version": "1.0.0",
                "mappings": [],
                "declared_counts": {
                    "typed_fields": 2,
                    "concrete_union_fields": 0,
                    "field_groups": 0,
                    "validation_rules": 1,
                    "calculations": 0,
                    "workflow_states": 0,
                    "workflow_transitions": 0,
                    "negative_fixtures": 0,
                    "confirmed_official_bugs": 0,
                    "unverified_gaps": 0
                }
            },
            "context_values": [{
                "context_value_id": "filing-year",
                "value_type": "integer",
                "required": true,
                "source_refs": [{"source_id": "review"}]
            }],
            "field_groups": [{
                "group_id": "rows",
                "min_occurs": 0,
                "max_occurs": 2,
                "instance_identity": "assigned-uuid",
                "members": ["row-note"],
                "source_refs": [{"source_id": "review"}]
            }],
            "fields": [{
                "field_id": "amount",
                "value_type": "decimal",
                "control_kind": "currency",
                "requiredness": "required",
                "group_id": null,
                "calculation_id": null,
                "serialized": [],
                "behavior": {
                    "official": {
                        "state": "executable",
                        "normalization": [{"kind": "trim", "side": "both"}],
                        "coercion": {
                            "kind": "decimal",
                            "decimal": {
                                "precision": decimal_precision,
                                "scale": decimal_scale,
                                "division_scale": 6,
                                "rounding": {"mode": "half-up", "scale": decimal_scale},
                                "overflow": "error"
                            },
                            "grouping": "comma",
                            "on_empty": "zero",
                            "on_invalid": if effect_kind == "coercion-failed" {
                                "preserve-raw"
                            } else {
                                "error"
                            }
                        },
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    },
                    "filing_safe": {
                        "state": "executable",
                        "normalization": [{"kind": "trim", "side": "both"}],
                        "coercion": {
                            "kind": "decimal",
                            "decimal": {
                                "precision": decimal_precision,
                                "scale": decimal_scale,
                                "division_scale": 6,
                                "rounding": {"mode": "half-up", "scale": decimal_scale},
                                "overflow": "error"
                            },
                            "grouping": "comma",
                            "on_empty": "zero",
                            "on_invalid": if effect_kind == "coercion-failed" {
                                "preserve-raw"
                            } else {
                                "error"
                            }
                        },
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    }
                },
                "source_refs": [{"source_id": "review"}]
            }, {
                "field_id": "row-note",
                "value_type": "string",
                "control_kind": "text",
                "requiredness": "optional",
                "group_id": "rows",
                "calculation_id": null,
                "serialized": [],
                "behavior": {
                    "official": {
                        "state": "executable",
                        "normalization": [],
                        "coercion": {"kind": "string", "on_empty": "empty-string"},
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    },
                    "filing_safe": {
                        "state": "executable",
                        "normalization": [],
                        "coercion": {"kind": "string", "on_empty": "empty-string"},
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    }
                },
                "source_refs": [{"source_id": "review"}]
            }],
            "evaluation_order": [],
            "calculations": [],
            "rules": [{
                "rule_id": "amount-nonnegative",
                "scope": {"kind": "singleton"},
                "order": 1,
                "phases": ["final-copy"],
                "field_ids": ["amount"],
                "profiles": {
                    "official": {
                        "state": "executable",
                        "predicate": official_predicate,
                        "effects": [effect.clone()],
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    },
                    "filing_safe": {
                        "state": "executable",
                        "predicate": {"kind": "constant", "value": true},
                        "effects": [effect],
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    }
                },
                "source_refs": [{"source_id": "review"}]
            }],
            "workflow": {
                "state": "unresolved",
                "reason": "synthetic generator fixture has no workflow",
                "source_refs": [{"source_id": "review"}]
            },
            "serialization": {
                "contract_version": "1.0.0",
                "artifacts": [{
                    "artifact_id": "synthetic-editable-save",
                    "target": "editable-save",
                    "variant_id": "synthetic-save",
                    "official": {
                        "state": "executable",
                        "nodes": [{
                            "kind": "pseudo-xml-field",
                            "ordinal": 1,
                            "key_projection": {"kind": "exact", "key": "amount"},
                            "occurrence_projection": {"kind": "fixed", "occurrence": 1},
                            "value_projection": {
                                "kind": "field",
                                "field": {
                                    "field_id": "amount",
                                    "instance": {"kind": "singleton"}
                                }
                            },
                            "semantic_format": {
                                "absent": "reject",
                                "blank": "reject",
                                "present": {
                                    "kind": "decimal",
                                    "scale": 2,
                                    "rounding": "half-up",
                                    "grouping": "comma",
                                    "decimal_separator": "period",
                                    "negative": "parentheses"
                                }
                            },
                            "body_codec": "legacy-javascript-escape",
                            "presence": {"kind": "always"},
                            "source_refs": [{"source_id": "review"}]
                        }, {
                            "kind": "metadata-element",
                            "ordinal": 2,
                            "exact_tag": "filingYear",
                            "value_projection": {
                                "kind": "context",
                                "context_value_id": "filing-year"
                            },
                            "semantic_format": {
                                "absent": "reject",
                                "blank": "reject",
                                "present": {"kind": "base10-integer"}
                            },
                            "body_codec": "raw-literal",
                            "presence": {
                                "kind": "when",
                                "predicate": {"kind": "constant", "value": true}
                            },
                            "source_refs": [{"source_id": "review"}]
                        }, {
                            "kind": "reviewed-literal",
                            "ordinal": 3,
                            "exact_bytes": [10, 60],
                            "review_decision": {"source_id": "review"},
                            "source_refs": [{"source_id": "review"}]
                        }, {
                            "kind": "dynamic-group",
                            "ordinal": 4,
                            "group_id": "rows",
                            "instance_order": "stable-instance-id-ascending",
                            "min_occurs": 0,
                            "max_occurs": 2,
                            "nodes": [{
                                "kind": "pseudo-xml-field",
                                "ordinal": 5,
                                "key_projection": {
                                    "kind": "group-indexed",
                                    "group_id": "rows",
                                    "index_base": 1,
                                    "index_step": 1,
                                    "padding": 2,
                                    "prefix": "row",
                                    "suffix": "Note",
                                    "review_decision": {"source_id": "review"},
                                    "source_refs": [{"source_id": "review"}]
                                },
                                "occurrence_projection": {
                                    "kind": "fixed",
                                    "occurrence": 1
                                },
                                "value_projection": {
                                    "kind": "field",
                                    "field": {
                                        "field_id": "row-note",
                                        "instance": {"kind": "current-group-instance"}
                                    }
                                },
                                "semantic_format": {
                                    "absent": "omit-occurrence",
                                    "blank": "emit-empty-body",
                                    "present": {"kind": "text"}
                                },
                                "body_codec": "raw-literal",
                                "presence": {"kind": "omitted"},
                                "source_refs": [{"source_id": "review"}]
                            }],
                            "review_decision": {"source_id": "review"},
                            "source_refs": [{"source_id": "review"}]
                        }],
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    },
                    "filing_safe": {
                        "state": "executable",
                        "nodes": [{
                            "kind": "pseudo-xml-field",
                            "ordinal": 1,
                            "key_projection": {"kind": "exact", "key": "amount"},
                            "occurrence_projection": {"kind": "fixed", "occurrence": 1},
                            "value_projection": {
                                "kind": "default",
                                "value": {"type": "decimal", "value": "0.00"},
                                "review_decision": {"source_id": "review"},
                                "source_refs": [{"source_id": "review"}]
                            },
                            "semantic_format": {
                                "absent": "reject",
                                "blank": "reject",
                                "present": {
                                    "kind": "decimal",
                                    "scale": 2,
                                    "rounding": "none",
                                    "grouping": "none",
                                    "decimal_separator": "period",
                                    "negative": "leading-minus"
                                }
                            },
                            "body_codec": "utf8-percent-rfc3986-unreserved",
                            "presence": {"kind": "always"},
                            "source_refs": [{"source_id": "review"}]
                        }],
                        "review_decision": {"source_id": "review"},
                        "source_refs": [{"source_id": "review"}]
                    },
                    "source_refs": [{"source_id": "review"}]
                }]
            },
            "fixtures": []
        });
        let canonical_rule_set = {
            let value: JsonValue = serde_json::from_value(document_json.clone()).unwrap();
            canonical_bytes(&value)
        };
        let serialization_contract_sha256 = {
            let value: JsonValue = serde_json::from_value(
                document_json
                    .get("serialization")
                    .expect("synthetic serialization")
                    .clone(),
            )
            .unwrap();
            crate::hash::sha256_hex(&canonical_bytes(&value))
        };
        let document: RuleSetDocument = serde_json::from_value(document_json).unwrap();
        let fixtures = std::collections::BTreeMap::from([(
            "synthetic-evaluation.json".to_owned(),
            synthetic_evaluation_fixture(rule_set_id, form_code, DIGEST),
        )]);
        AuditedSnapshot {
            index: IndexSnapshot {
                rule_set_id: rule_set_id.to_owned(),
                form_code: form_code.to_owned(),
                form_revision: "2024-01-01".to_owned(),
                official_package_version: "1.0.0".to_owned(),
                source_set_sha256: Some(DIGEST.to_owned()),
                path: format!("{rule_set_id}/rule-set.json"),
                review_status: ReviewStatus::Reviewed,
                profile_states: ProfileStates {
                    official: BranchState::Executable,
                    filing_safe: BranchState::Executable,
                },
            },
            document,
            canonical_rule_set,
            serialization_contract_sha256,
            normalized_source_sha256: DIGEST.to_owned(),
            fixtures,
        }
    }

    fn synthetic_evaluation_fixture(
        rule_set_id: &str,
        form_code: &str,
        source_set_sha256: &str,
    ) -> JsonValue {
        let context_values: JsonValue = serde_json::from_value(json!({
            "values": [{
                "id": "filing-year",
                "value": {"type": "integer", "value": 2024}
            }]
        }))
        .expect("synthetic context values deserialize");
        let mut fingerprint_input = b"bir-rules/context-value-snapshot/v1\0".to_vec();
        fingerprint_input.extend(canonical_bytes(&context_values));
        let context_fingerprint = crate::hash::sha256_hex(&fingerprint_input);
        let row_instance = json!({
            "group_id": "rows",
            "instance_id": "sentinel-row-007"
        });
        let amount_raw = json!({"state": "text", "text": "-1.00"});
        let row_note_raw = json!({
            "state": "text",
            "text": "SENTINEL fixture canonical input value"
        });

        serde_json::from_value(json!({
            "$schema": "../../../../schema/v2/fixture.schema.json",
            "schema_version": "2.0.0",
            "fixture_id": "synthetic-reviewed-evaluation",
            "kind": "evaluation",
            "description": "Synthetic reviewed fixture emission sentinel",
            "source_refs": [{"source_id": "review"}],
            "input": {
                "rule_set": {
                    "rule_set_id": rule_set_id,
                    "form_code": form_code,
                    "form_revision": "2024-01-01",
                    "official_package_version": "1.0.0",
                    "source_set_sha256": source_set_sha256
                },
                "context": {
                    "phase": "final-copy",
                    "profile": "official"
                },
                "input_revision": 7,
                "context_fingerprint": context_fingerprint,
                "context_values": context_values,
                "raw_inputs": {
                    "repeated_group_instances": [row_instance.clone()],
                    "fields": [{
                        "field": {
                            "field_id": "amount",
                            "group_path": []
                        },
                        "value": amount_raw.clone()
                    }, {
                        "field": {
                            "field_id": "row-note",
                            "group_path": [row_instance.clone()]
                        },
                        "value": row_note_raw.clone()
                    }]
                }
            },
            "expected": {
                "report": {
                    "rule_set": {
                        "rule_set_id": rule_set_id,
                        "form_code": form_code,
                        "form_revision": "2024-01-01",
                        "official_package_version": "1.0.0",
                        "source_set_sha256": source_set_sha256
                    },
                    "context": {
                        "phase": "final-copy",
                        "profile": "official"
                    },
                    "input_revision": 7,
                    "context_fingerprint": context_fingerprint,
                    "expected_rules": [{
                        "execution": {
                            "rule_id": "amount-nonnegative",
                            "instance": null
                        },
                        "order": 1
                    }],
                    "evaluated_rules": [{
                        "rule_id": "amount-nonnegative",
                        "instance": null
                    }],
                    "violations": [{
                        "execution": {
                            "rule_id": "amount-nonnegative",
                            "instance": null
                        },
                        "phase": "final-copy",
                        "order": {
                            "rule_order": 1,
                            "occurrence": 0
                        },
                        "fields": [{
                            "field": {
                                "field_id": "amount",
                                "group_path": []
                            },
                            "xml_key": null,
                            "serialized_occurrence": null
                        }],
                        "official_message": null,
                        "message": "SENTINEL fixture violation message.",
                        "assessment": "verified-correct",
                        "severity": "blocking",
                        "profile": "official"
                    }]
                },
                "canonical_inputs": [{
                    "field": {
                        "field_id": "amount",
                        "group_path": []
                    },
                    "raw": amount_raw,
                    "canonical": {
                        "type": "decimal",
                        "value": "-1"
                    }
                }, {
                    "field": {
                        "field_id": "row-note",
                        "group_path": [row_instance]
                    },
                    "raw": row_note_raw,
                    "canonical": {
                        "type": "text",
                        "value": "SENTINEL fixture canonical input value"
                    }
                }],
                "expected_outputs": [],
                "derived_outputs": []
            }
        }))
        .expect("synthetic evaluation fixture deserializes")
    }
}
