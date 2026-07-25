//! Deterministic re-projection of the 2550Q v2 candidate static field surface.
//!
//! This replaces `rules/tools/update-2550q-v2-static-projections.ps1`, which
//! could only ever run under Windows PowerShell, for the same two reasons the
//! sibling binding builder could not:
//!
//! * It joined all five input paths with a literal `\` separator, so on any
//!   other platform every input resolved to a single non-existent file name.
//!   Every path here is resolved through [`crate::path`], which rejects `\`
//!   outright, so that defect class cannot recur.
//! * It appended `[Environment]::NewLine` to every emitted document, so the
//!   artifacts' bytes depended on the operating system that produced them.
//!   This module always emits LF with exactly one trailing newline, UTF-8
//!   without a BOM, and sorted object keys.
//!
//! The projector is idempotent: it recomputes the 34 executable raw candidate
//! fields from evidence, appends them to the 60 fields that carry no projection
//! source reference, and rewrites the 121 evaluation fixtures so every
//! singleton field has a raw input and a canonical expectation. Running it
//! against a corpus it already produced reproduces that corpus, which is the
//! success oracle `static_projection_reproduces_the_tracked_corpus` asserts.
//!
//! The port is a fidelity port: every count, assertion message, literal and
//! emitted value is transcribed unchanged. Three things are deliberately *not*
//! carried over, all of them unreachable or untrue in the original:
//!
//! * the `workflow`/`derived` requiredness, `official` documented-only, and
//!   `control_kind` branches (PowerShell lines 160-166, 209-237 and 241-252)
//!   are all dead past the `continue` at line 159, which skips every
//!   non-`raw` category before they can be evaluated;
//! * the closing message at line 384 reports "34 executable raw fields, 53
//!   identity-only documented controls" as if 87 fields were emitted, when
//!   only the 34 raw ones ever reach the rule set. The message here says what
//!   actually happened.
//!
//! The sibling `update-2550q-v2-group-projections.ps1` is deliberately not
//! ported. It asserts a post-condition of 60 total fields against a corpus that
//! now carries 94, so it throws before writing anything and can never run
//! again; it is archaeology, not a tool.
//!
//! Two behavioural narrowings, both inert on the pinned corpus:
//!
//! * PowerShell compares strings case-insensitively by default (`-contains`,
//!   `-eq`, and hashtable lookups all ignore case). Identifier matching here is
//!   ordinal. No two control ids, field keys or field ids in the corpus collide
//!   case-insensitively, so nothing resolves differently.
//! * PowerShell coerces a missing or non-string property to the empty string
//!   rather than failing. Missing structure fails here instead. The one place
//!   where the coercion is load-bearing — `[string]$raw.text` on a value that
//!   carries no `text` — is reproduced exactly.
//!
//! The emitted identity order is **ordinal**, transcribed from
//! `[StringComparer]::Ordinal`: `BTreeMap<String, _>` orders by UTF-8 bytes, so
//! every uppercase ASCII letter sorts before every lowercase one. A
//! locale-aware or case-insensitive sort would reorder the fixtures' field
//! lists wholesale.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{CodegenError, Result};
use crate::files::read_bytes;
use crate::json::{JsonValue, parse_strict};
use crate::path::{
    canonical_repo_root, is_json_file, resolve_existing_under, resolve_output_under,
};

const RULE_SET_PATH: &str = "rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json";
const FIXTURES_PATH: &str = "rules/ir/v2/2550q-v2024-p7.9.6.0/fixtures";
const BINDING_INVENTORY_PATH: &str =
    "rules/forms/2550q-v2024/fixtures/serialization-binding-inventory-v796.json";
const RUNTIME_CONTROL_INVENTORY_PATH: &str =
    "rules/forms/2550q-v2024/fixtures/runtime-control-inventory-v796.json";
const V1_FIELDS_PATH: &str = "rules/forms/2550q-v2024/fields.json";
/// Read as text, not as structure: the projector's raw/derived split is exactly
/// "does the core adapter mention this control id as a quoted string literal".
const CORE_ADAPTER_PATH: &str = "crates/bir-core/src/form_rules/form_2550q.rs";

const PROJECTION_SOURCE_ID: &str = "candidate-static-surface-projection-review";
const RAW_LEXICAL_LOCATOR: &str = "#raw-lexical-controls";
const CANDIDATE_ONLY_LOCATOR: &str = "#candidate-only-boundary";
const STATIC_CONTROL_PROJECTION_KIND: &str = "raw-static-control";

const FILING_SAFE_REASON: &str = "No independent filing-safe field, derivation, workflow-state, or serialization policy has been reviewed.";

/// The four radio controls whose exact core raw binding was reviewed by hand.
/// Losing one of them silently would move a filing answer into the
/// documented-only surface, so their survival is asserted, not assumed.
const REVIEWED_RADIO_KEYS: [&str; 4] = [
    "frm2550qv2024:amendedReturnYesNo5",
    "frm2550qv2024:amendedReturnNo5",
    "frm2550qv2024:OptShortPrd1",
    "frm2550qv2024:OptShortPrd2",
];

/// Package workflow, credential and UI-state controls. These are identified but
/// never projected into the executable field surface.
const WORKFLOW_KEYS: [&str; 9] = [
    "driveSelectTPExport",
    "ebirOnlineConfirmUsername",
    "ebirOnlineSecret",
    "ebirOnlineUsername",
    "frm2550qv2024:txtCurrentPage",
    "frm2550qv2024:txtMaxPage",
    "txtEmail",
    "txtEnroll",
    "txtFinalFlag",
];

const REQUIREDNESS_VALUES: [&str; 5] =
    ["required", "optional", "conditional", "computed", "hidden"];

/// The published decomposition of the static surface. Never adjust these to
/// make a run pass; they are quoted as fact in the 2550Q review documents.
const EXPECTED_TARGET_COUNT: usize = 87;
const EXPECTED_RAW_COUNT: usize = 34;
const EXPECTED_DERIVED_COUNT: usize = 44;
const EXPECTED_WORKFLOW_COUNT: usize = 9;
const EXPECTED_FIELD_COUNT: usize = 94;

#[derive(Clone, Debug)]
pub struct ProjectStaticSurfaceOptions {
    pub repo_root: PathBuf,
    /// Repository-relative directory to project into instead of the canonical
    /// corpus.
    ///
    /// `rules/UPDATING.md:33-36` requires a staging root and a
    /// fail-if-target-exists guard before a builder is pointed at a new
    /// release, because these writers otherwise overwrite a historical snapshot
    /// in place. Writing into the canonical corpus is only defensible for
    /// idempotent regeneration of the snapshot that is already there — which is
    /// why that stays the default, and why staging refuses to overwrite.
    pub staging_root: Option<String>,
}

impl ProjectStaticSurfaceOptions {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            staging_root: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StaticProjectionReport {
    pub rule_set_path: PathBuf,
    /// Raw candidate fields appended to the executable surface.
    pub executable_raw_field_count: usize,
    /// Derived/alias plus workflow controls that stay identity-only. These are
    /// counted, asserted, and then deliberately not emitted.
    pub documented_only_control_count: usize,
    pub total_field_count: usize,
    pub fixture_count: usize,
}

/// Recomputes the 2550Q v2 static field surface and rewrites the rule set and
/// every evaluation fixture under it.
///
/// Writes exactly as the PowerShell original did: the rule set first, then each
/// fixture in name order, each through a staged sibling and a rename. A failure
/// part-way therefore leaves earlier files already rewritten — harmless,
/// because the projection is idempotent and re-running completes it.
pub fn project_2550q_static_surface(
    options: &ProjectStaticSurfaceOptions,
) -> Result<StaticProjectionReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let plan = build_static_projection(&repo_root)?;

    let staging = match &options.staging_root {
        Some(relative) => Some(resolve_output_under(
            &repo_root,
            relative,
            "static projection staging root",
        )?),
        None => None,
    };
    let target = |path: &Path| -> Result<PathBuf> {
        let Some(staging) = &staging else {
            return Ok(path.to_owned());
        };
        let relative = path.strip_prefix(&repo_root).map_err(|_| {
            CodegenError::new(format!(
                "projected path `{}` is not under the repository root",
                path.display()
            ))
        })?;
        let destination = staging.join(relative);
        // The guard UPDATING.md asks for: staging must never silently replace
        // an artifact that a previous run already produced there.
        if destination.exists() {
            return Err(CodegenError::new(format!(
                "staged output `{}` already exists; refusing to overwrite a previous projection",
                destination.display()
            )));
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|source| {
                CodegenError::io("create staged output directory", parent, source)
            })?;
        }
        Ok(destination)
    };

    write_atomically(&target(&plan.rule_set_path)?, &render(&plan.rule_set)?)?;
    for fixture_path in &plan.fixture_paths {
        let mut fixture = read_json(fixture_path)?;
        project_fixture(&mut fixture, &plan.singleton_fields, fixture_path)?;
        write_atomically(&target(fixture_path)?, &render(&fixture)?)?;
    }

    Ok(StaticProjectionReport {
        rule_set_path: plan.rule_set_path,
        executable_raw_field_count: plan.executable_raw_field_count,
        documented_only_control_count: plan.documented_only_control_count,
        total_field_count: plan.total_field_count,
        fixture_count: plan.fixture_paths.len(),
    })
}

/// One singleton (`group_id: null`) candidate field, reduced to exactly what
/// the fixture rewrite needs.
struct SingletonField {
    field_id: String,
    is_boolean: bool,
    /// Whether this field carries a projection source reference. Static
    /// projections have their canonical expectation recomputed from the raw
    /// input on every run; everything else keeps whatever a reviewer recorded.
    is_static_projection: bool,
}

struct StaticProjection {
    rule_set_path: PathBuf,
    rule_set: JsonValue,
    fixture_paths: Vec<PathBuf>,
    singleton_fields: Vec<SingletonField>,
    executable_raw_field_count: usize,
    documented_only_control_count: usize,
    total_field_count: usize,
}

fn build_static_projection(repo_root: &Path) -> Result<StaticProjection> {
    let rule_set_path = resolve_input(repo_root, RULE_SET_PATH)?;
    let fixtures_dir = resolve_input(repo_root, FIXTURES_PATH)?;
    let binding_path = resolve_input(repo_root, BINDING_INVENTORY_PATH)?;
    let runtime_path = resolve_input(repo_root, RUNTIME_CONTROL_INVENTORY_PATH)?;
    let v1_fields_path = resolve_input(repo_root, V1_FIELDS_PATH)?;
    let core_adapter_path = resolve_input(repo_root, CORE_ADAPTER_PATH)?;

    let mut rule_set = read_json(&rule_set_path)?;
    let binding = read_json(&binding_path)?;
    let runtime = read_json(&runtime_path)?;
    let v1_fields = read_json(&v1_fields_path)?;
    let core_adapter = read_text(&core_adapter_path)?;
    let core_singleton_field_ids = parse_core_singleton_field_ids(&core_adapter)?;

    // The original calls this `$existingProjectedIds`; it is in fact the set of
    // ids that carry *no* projection source reference, i.e. the reviewed fields
    // this run must preserve untouched.
    let mut preserved_fields: Vec<JsonValue> = Vec::new();
    let mut preserved_field_ids: BTreeSet<&str> = BTreeSet::new();
    for field in array_at(&rule_set, "fields", RULE_SET_PATH)? {
        if has_projection_source_ref(field) {
            continue;
        }
        preserved_field_ids.insert(required_string(field, "field_id", RULE_SET_PATH)?);
        preserved_fields.push(field.clone());
    }

    let occurrences = array_at(&binding, "occurrence_bindings", BINDING_INVENTORY_PATH)?;
    let mut target_keys: Vec<&str> = Vec::new();
    for occurrence in occurrences {
        if projection_kind(occurrence) != Some(STATIC_CONTROL_PROJECTION_KIND) {
            continue;
        }
        let key = required_string(occurrence, "key", BINDING_INVENTORY_PATH)?;
        if preserved_field_ids.contains(key) {
            continue;
        }
        target_keys.push(key);
    }
    let target_key_set: BTreeSet<&str> = target_keys.iter().copied().collect();
    if target_keys.len() != EXPECTED_TARGET_COUNT || target_key_set.len() != EXPECTED_TARGET_COUNT {
        return Err(CodegenError::new(format!(
            "Expected {EXPECTED_TARGET_COUNT} unique unprojected static controls; found {}.",
            target_keys.len()
        )));
    }

    let runtime_controls = array_at(&runtime, "static_controls", RUNTIME_CONTROL_INVENTORY_PATH)?;
    let runtime_index_by_id = index_by(
        runtime_controls,
        "id",
        &target_key_set,
        "Duplicate runtime control identity",
    )?;
    let v1_field_list = array_at(&v1_fields, "fields", V1_FIELDS_PATH)?;
    let v1_index_by_key = index_by(
        v1_field_list,
        "field_key",
        &target_key_set,
        "Duplicate v1 field identity",
    )?;
    let binding_index_by_key = index_by(
        occurrences,
        "key",
        &target_key_set,
        "Duplicate observed static occurrence identity",
    )?;

    let mut raw_keys: BTreeSet<&str> = BTreeSet::new();
    let mut derived_keys: BTreeSet<&str> = BTreeSet::new();
    for key in &target_keys {
        if !runtime_index_by_id.contains_key(key)
            || !v1_index_by_key.contains_key(key)
            || !binding_index_by_key.contains_key(key)
        {
            return Err(CodegenError::new(format!(
                "Static projection lacks exact evidence indices: {key}"
            )));
        }
        if core_singleton_field_ids.contains(*key) {
            raw_keys.insert(key);
        } else if !WORKFLOW_KEYS.contains(key) {
            derived_keys.insert(key);
        }
    }
    let workflow_target_count = target_keys
        .iter()
        .filter(|key| WORKFLOW_KEYS.contains(*key))
        .count();
    if raw_keys.len() != EXPECTED_RAW_COUNT
        || derived_keys.len() != EXPECTED_DERIVED_COUNT
        || workflow_target_count != EXPECTED_WORKFLOW_COUNT
    {
        return Err(CodegenError::new(format!(
            "Static projection category counts changed (raw={}, derived={}).",
            raw_keys.len(),
            derived_keys.len()
        )));
    }
    for key in REVIEWED_RADIO_KEYS.iter().chain(WORKFLOW_KEYS.iter()) {
        if !target_key_set.contains(key) {
            return Err(CodegenError::new(format!(
                "Reviewed static category key disappeared: {key}"
            )));
        }
    }
    for key in REVIEWED_RADIO_KEYS {
        if !raw_keys.contains(key) {
            return Err(CodegenError::new(format!(
                "Reviewed radio control lost its exact core raw binding: {key}"
            )));
        }
    }

    let mut fields = preserved_fields;
    let mut executable_raw_field_count = 0;
    for key in &target_keys {
        // Only executable raw captures belong in the candidate field surface.
        // Derived/alias and workflow identities remain bound in the value-free
        // occurrence inventory until their executable behavior is reviewed.
        if !raw_keys.contains(key) {
            continue;
        }
        let runtime_index = runtime_index_by_id[key];
        let v1_index = v1_index_by_key[key];
        let binding_index = binding_index_by_key[key];
        let control = &runtime_controls[runtime_index];
        let v1_field = &v1_field_list[v1_index];

        let requiredness = requiredness(v1_field, key)?;
        let control_kind =
            required_string(control, "control_kind", RUNTIME_CONTROL_INVENTORY_PATH)?;
        let is_boolean = control_kind == "radio";

        fields.push(object([
            ("field_id", string(*key)),
            (
                "value_type",
                string(if is_boolean { "boolean" } else { "string" }),
            ),
            ("control_kind", string(control_kind)),
            ("requiredness", string(requiredness)),
            ("group_id", JsonValue::Null),
            ("calculation_id", JsonValue::Null),
            ("serialized", JsonValue::Array(Vec::new())),
            (
                "behavior",
                object([
                    (
                        "official",
                        object([
                            ("state", string("executable")),
                            ("normalization", JsonValue::Array(Vec::new())),
                            ("coercion", coercion(is_boolean)),
                            (
                                "review_decision",
                                source_ref(PROJECTION_SOURCE_ID, RAW_LEXICAL_LOCATOR),
                            ),
                            (
                                "source_refs",
                                JsonValue::Array(vec![
                                    source_ref(
                                        "v1-serialization-binding-inventory",
                                        format!("#/occurrence_bindings/{binding_index}"),
                                    ),
                                    source_ref(
                                        "v1-runtime-control-inventory",
                                        format!("#/static_controls/{runtime_index}"),
                                    ),
                                    source_ref(PROJECTION_SOURCE_ID, RAW_LEXICAL_LOCATOR),
                                ]),
                            ),
                        ]),
                    ),
                    (
                        "filing_safe",
                        object([
                            ("state", string("unresolved")),
                            ("reason", string(FILING_SAFE_REASON)),
                            (
                                "source_refs",
                                JsonValue::Array(vec![source_ref(
                                    PROJECTION_SOURCE_ID,
                                    CANDIDATE_ONLY_LOCATOR,
                                )]),
                            ),
                        ]),
                    ),
                ]),
            ),
            (
                "source_refs",
                JsonValue::Array(vec![
                    source_ref("v1-fields", format!("#/fields/{v1_index}")),
                    source_ref(
                        "v1-runtime-control-inventory",
                        format!("#/static_controls/{runtime_index}"),
                    ),
                    source_ref(
                        "v1-serialization-binding-inventory",
                        format!("#/occurrence_bindings/{binding_index}"),
                    ),
                    source_ref(PROJECTION_SOURCE_ID, CANDIDATE_ONLY_LOCATOR),
                ]),
            ),
        ]));
        executable_raw_field_count += 1;
    }

    if fields.len() != EXPECTED_FIELD_COUNT {
        return Err(CodegenError::new(format!(
            "Expected {EXPECTED_FIELD_COUNT} executable candidate fields after static projection; found {}.",
            fields.len()
        )));
    }
    let mut field_ids: BTreeSet<&str> = BTreeSet::new();
    for field in &fields {
        field_ids.insert(required_string(field, "field_id", RULE_SET_PATH)?);
    }
    if field_ids.len() != EXPECTED_FIELD_COUNT {
        return Err(CodegenError::new(
            "Static projection introduced duplicate candidate field identities.",
        ));
    }

    let mut singleton_fields = Vec::new();
    for field in &fields {
        if !matches!(
            field.object().and_then(|field| field.get("group_id")),
            None | Some(JsonValue::Null)
        ) {
            continue;
        }
        singleton_fields.push(SingletonField {
            field_id: required_string(field, "field_id", RULE_SET_PATH)?.to_owned(),
            is_boolean: field
                .object()
                .and_then(|field| field.get("value_type"))
                .and_then(JsonValue::as_str)
                == Some("boolean"),
            is_static_projection: has_projection_source_ref(field),
        });
    }
    // Sorted only to keep the traversal reproducible; identities are unique, so
    // the order in which singletons are visited cannot change any emitted list.
    singleton_fields.sort_by(|left, right| left.field_id.cmp(&right.field_id));

    let total_field_count = fields.len();
    rule_set
        .object_mut()
        .ok_or_else(|| CodegenError::new(format!("{RULE_SET_PATH} is not a JSON object")))?
        .insert("fields".to_owned(), JsonValue::Array(fields));

    Ok(StaticProjection {
        rule_set_path,
        rule_set,
        fixture_paths: fixture_paths(&fixtures_dir)?,
        singleton_fields,
        executable_raw_field_count,
        documented_only_control_count: derived_keys.len() + workflow_target_count,
        total_field_count,
    })
}

/// Gives every singleton field a raw input and a canonical expectation, then
/// re-emits both lists in ordinal identity order.
fn project_fixture(
    fixture: &mut JsonValue,
    singleton_fields: &[SingletonField],
    path: &Path,
) -> Result<()> {
    let context = path.display().to_string();
    let raw_list = nested_array(fixture, RAW_INPUT_PATH, &context)?.clone();
    let canonical_list = nested_array(fixture, CANONICAL_INPUT_PATH, &context)?.clone();

    let mut raw_by_id: BTreeMap<String, JsonValue> = BTreeMap::new();
    for item in raw_list {
        raw_by_id.insert(item_field_id(&item, &context)?, item);
    }
    let mut canonical_by_id: BTreeMap<String, JsonValue> = BTreeMap::new();
    for item in canonical_list {
        canonical_by_id.insert(item_field_id(&item, &context)?, item);
    }

    for field in singleton_fields {
        let field_id = field.field_id.as_str();
        if !raw_by_id.contains_key(field_id) {
            let text = if field.is_boolean { "false" } else { "" };
            raw_by_id.insert(
                field.field_id.clone(),
                object([
                    ("field", field_identity(field_id)),
                    (
                        "value",
                        object([("state", string("text")), ("text", string(text))]),
                    ),
                ]),
            );
        }
        if canonical_by_id.contains_key(field_id) && !field.is_static_projection {
            continue;
        }

        let raw = raw_by_id[field_id]
            .object()
            .and_then(|item| item.get("value"))
            .cloned()
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "{context} raw input `{field_id}` is missing object `value`"
                ))
            })?;
        let state = raw
            .object()
            .and_then(|value| value.get("state"))
            .and_then(JsonValue::as_str);
        // `[string]$raw.text` on an absent property is the empty string, and an
        // absent raw value legitimately carries no text at all.
        let text = raw
            .object()
            .and_then(|value| value.get("text"))
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        let canonical = if state == Some("absent") {
            object([("type", string("absent"))])
        } else if field.is_boolean {
            object([
                ("type", string("boolean")),
                ("value", JsonValue::Bool(text == "true")),
            ])
        } else {
            object([("type", string("text")), ("value", string(text))])
        };
        canonical_by_id.insert(
            field.field_id.clone(),
            object([
                ("field", field_identity(field_id)),
                ("raw", raw),
                ("canonical", canonical),
            ]),
        );
    }

    // `BTreeMap` iteration is UTF-8 byte order, which is what
    // `[StringComparer]::Ordinal` sorts by.
    let raw_items = raw_by_id.into_values().collect();
    let canonical_items = canonical_by_id.into_values().collect();
    set_nested(
        fixture,
        RAW_INPUT_PATH,
        JsonValue::Array(raw_items),
        &context,
    )?;
    set_nested(
        fixture,
        CANONICAL_INPUT_PATH,
        JsonValue::Array(canonical_items),
        &context,
    )
}

const RAW_INPUT_PATH: &[&str] = &["input", "raw_inputs", "fields"];
const CANONICAL_INPUT_PATH: &[&str] = &["expected", "canonical_inputs"];

fn coercion(is_boolean: bool) -> JsonValue {
    if is_boolean {
        object([
            ("kind", string("boolean")),
            ("true_values", JsonValue::Array(vec![string("true")])),
            ("false_values", JsonValue::Array(vec![string("false")])),
            ("on_empty", string("error")),
            ("on_invalid", string("error")),
        ])
    } else {
        object([
            ("kind", string("string")),
            ("on_empty", string("empty-string")),
        ])
    }
}

fn requiredness<'a>(v1_field: &'a JsonValue, key: &str) -> Result<&'a str> {
    let declared = v1_field
        .object()
        .and_then(|field| field.get("required"))
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    if !REQUIREDNESS_VALUES.contains(&declared) {
        return Err(CodegenError::new(format!(
            "Unsupported requiredness {declared} for {key}"
        )));
    }
    Ok(declared)
}

fn source_ref(source_id: &str, locator: impl Into<String>) -> JsonValue {
    object([
        ("source_id", string(source_id)),
        ("locator", JsonValue::String(locator.into())),
    ])
}

fn field_identity(field_id: &str) -> JsonValue {
    object([
        ("field_id", string(field_id)),
        ("group_path", JsonValue::Array(Vec::new())),
    ])
}

fn has_projection_source_ref(field: &JsonValue) -> bool {
    field
        .object()
        .and_then(|field| field.get("source_refs"))
        .and_then(|refs| match refs {
            JsonValue::Array(values) => Some(values),
            _ => None,
        })
        .is_some_and(|refs| {
            refs.iter().any(|source_ref| {
                source_ref
                    .object()
                    .and_then(|source_ref| source_ref.get("source_id"))
                    .and_then(JsonValue::as_str)
                    == Some(PROJECTION_SOURCE_ID)
            })
        })
}

fn projection_kind(occurrence: &JsonValue) -> Option<&str> {
    occurrence
        .object()
        .and_then(|occurrence| occurrence.get("source_projection"))
        .and_then(JsonValue::object)
        .and_then(|projection| projection.get("kind"))
        .and_then(JsonValue::as_str)
}

/// Builds a zero-based index of the entries whose `key` identity is one of the
/// projection targets, rejecting a repeated identity outright: a duplicate
/// would make the emitted evidence locator ambiguous.
fn index_by<'a>(
    entries: &'a [JsonValue],
    key: &str,
    wanted: &BTreeSet<&str>,
    duplicate_message: &str,
) -> Result<BTreeMap<&'a str, usize>> {
    let mut indexed = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let Some(identity) = entry
            .object()
            .and_then(|entry| entry.get(key))
            .and_then(JsonValue::as_str)
            .filter(|identity| !identity.is_empty())
        else {
            continue;
        };
        if !wanted.contains(identity) {
            continue;
        }
        if indexed.insert(identity, index).is_some() {
            return Err(CodegenError::new(format!(
                "{duplicate_message}: {identity}"
            )));
        }
    }
    Ok(indexed)
}

/// Non-recursive, files only, `*.json`, in name order: exactly what
/// `Get-ChildItem -Filter '*.json' -File | Sort-Object Name` enumerated.
fn fixture_paths(directory: &Path) -> Result<Vec<PathBuf>> {
    let entries = fs::read_dir(directory)
        .map_err(|source| CodegenError::io("read fixture directory", directory, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CodegenError::io("read fixture directory entry", directory, source))?;

    let mut paths = Vec::new();
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| CodegenError::io("read fixture file type", &path, source))?;
        if file_type.is_symlink() {
            return Err(CodegenError::new(format!(
                "refusing to rewrite symlinked fixture `{}`",
                path.display()
            )));
        }
        if file_type.is_file() && is_json_file(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn render(document: &JsonValue) -> Result<Vec<u8>> {
    let mut text = serde_json::to_string_pretty(document)
        .map_err(|source| CodegenError::with_source("serialize 2550Q static projection", source))?;
    text.push('\n');
    Ok(text.into_bytes())
}

/// Writes through a uniquely named sibling so a failed write can never leave a
/// truncated rule set or fixture in place.
/// The exact contents of `SINGLETON_FIELD_IDS` in the core adapter.
///
/// Whether a control becomes an executable raw field or stays a documented-only
/// projection is decided by whether `bir-core` actually binds it. The PowerShell
/// original answered that with `core_adapter.contains("\"<key>\"")` — a
/// substring search over the whole file. That file holds 297 distinct quoted
/// strings of which only 66 are field ids, so a doc comment or an error message
/// mentioning a control id would silently promote a documented-only projection
/// into the executable field surface. The 34/44/9 count assertions catch a net
/// change but not an offsetting swap.
///
/// Reading the declaration itself is the same signal without the ambiguity, and
/// it fails loudly rather than classifying everything as derived if the
/// declaration moves.
fn parse_core_singleton_field_ids(source: &str) -> Result<BTreeSet<String>> {
    const MARKER: &str = "const SINGLETON_FIELD_IDS";
    let start = source.find(MARKER).ok_or_else(|| {
        CodegenError::new(format!(
            "{CORE_ADAPTER_PATH} no longer declares {MARKER}; the raw/derived split has no signal"
        ))
    })?;
    let open = source[start..].find('[').ok_or_else(|| {
        CodegenError::new(format!(
            "{MARKER} in {CORE_ADAPTER_PATH} has no array literal"
        ))
    })? + start;
    let close = source[open..].find("];").ok_or_else(|| {
        CodegenError::new(format!("{MARKER} in {CORE_ADAPTER_PATH} is unterminated"))
    })? + open;

    let mut ids = BTreeSet::new();
    let body = &source[open..close];
    let mut rest = body;
    while let Some(quote) = rest.find('"') {
        rest = &rest[quote + 1..];
        let Some(end) = rest.find('"') else {
            break;
        };
        ids.insert(rest[..end].to_owned());
        rest = &rest[end + 1..];
    }
    if ids.is_empty() {
        return Err(CodegenError::new(format!(
            "{MARKER} in {CORE_ADAPTER_PATH} parsed to no field ids"
        )));
    }
    Ok(ids)
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| CodegenError::new(format!("output `{}` has no parent", path.display())))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            CodegenError::new(format!(
                "output name `{}` is not valid UTF-8",
                path.display()
            ))
        })?;
    let staging = parent.join(format!(
        "{name}.bir-rules-codegen-projections-{}",
        std::process::id()
    ));

    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    let mut file = options
        .open(&staging)
        .map_err(|source| CodegenError::io("create staged projection", &staging, source))?;
    file.write_all(bytes)
        .map_err(|source| CodegenError::io("write staged projection", &staging, source))?;
    file.sync_all()
        .map_err(|source| CodegenError::io("sync staged projection", &staging, source))?;
    drop(file);

    fs::rename(&staging, path).map_err(|source| {
        let _ = fs::remove_file(&staging);
        CodegenError::io("install projection", path, source)
    })
}

fn resolve_input(repo_root: &Path, relative: &str) -> Result<PathBuf> {
    resolve_existing_under(
        repo_root,
        relative,
        &format!("2550Q static projection input `{relative}`"),
    )
}

fn read_json(path: &Path) -> Result<JsonValue> {
    let bytes = read_bytes(path)?;
    parse_strict(&bytes, path)
}

fn read_text(path: &Path) -> Result<String> {
    let bytes = read_bytes(path)?;
    String::from_utf8(bytes).map_err(|source| {
        CodegenError::with_source(format!("`{}` is not valid UTF-8", path.display()), source)
    })
}

fn array_at<'a>(value: &'a JsonValue, key: &str, context: &str) -> Result<&'a Vec<JsonValue>> {
    value
        .object()
        .and_then(|value| value.get(key))
        .and_then(|value| match value {
            JsonValue::Array(values) => Some(values),
            _ => None,
        })
        .ok_or_else(|| CodegenError::new(format!("{context} is missing array `{key}`")))
}

fn nested_array<'a>(
    value: &'a JsonValue,
    path: &[&str],
    context: &str,
) -> Result<&'a Vec<JsonValue>> {
    let mut current = value;
    for (depth, key) in path.iter().enumerate() {
        if depth + 1 == path.len() {
            return array_at(current, key, context);
        }
        current = current
            .object()
            .and_then(|current| current.get(*key))
            .ok_or_else(|| CodegenError::new(format!("{context} is missing object `{key}`")))?;
    }
    unreachable!("a nested path always has a final key")
}

fn set_nested(
    value: &mut JsonValue,
    path: &[&str],
    replacement: JsonValue,
    context: &str,
) -> Result<()> {
    let mut current = value;
    for (depth, key) in path.iter().enumerate() {
        let object = current
            .object_mut()
            .ok_or_else(|| CodegenError::new(format!("{context} `{key}` has no object parent")))?;
        if depth + 1 == path.len() {
            object.insert((*key).to_owned(), replacement);
            return Ok(());
        }
        current = object
            .get_mut(*key)
            .ok_or_else(|| CodegenError::new(format!("{context} is missing object `{key}`")))?;
    }
    unreachable!("a nested path always has a final key")
}

fn item_field_id(item: &JsonValue, context: &str) -> Result<String> {
    item.object()
        .and_then(|item| item.get("field"))
        .and_then(JsonValue::object)
        .and_then(|field| field.get("field_id"))
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            CodegenError::new(format!("{context} has an entry without `field.field_id`"))
        })
}

fn required_string<'a>(value: &'a JsonValue, key: &str, context: &str) -> Result<&'a str> {
    value
        .object()
        .and_then(|value| value.get(key))
        .and_then(JsonValue::as_str)
        .ok_or_else(|| CodegenError::new(format!("{context} is missing string `{key}`")))
}

fn object(entries: impl IntoIterator<Item = (&'static str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn string(value: impl Into<String>) -> JsonValue {
    JsonValue::String(value.into())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        CORE_ADAPTER_PATH, FIXTURES_PATH, ProjectStaticSurfaceOptions, RULE_SET_PATH,
        fixture_paths, project_2550q_static_surface, read_json,
    };
    use crate::json::canonical_bytes;
    use crate::path::canonical_repo_root;

    static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

    const COPIED_INPUTS: [&str; 4] = [
        RULE_SET_PATH,
        super::BINDING_INVENTORY_PATH,
        super::RUNTIME_CONTROL_INVENTORY_PATH,
        super::V1_FIELDS_PATH,
    ];

    fn landed_repo_root() -> PathBuf {
        canonical_repo_root(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("repository root")
    }

    /// Copies exactly the documents the projector reads into a throwaway root.
    /// The projector rewrites what it reads, so no test may ever point it at the
    /// tracked corpus.
    fn materialize_scratch_corpus(repo_root: &Path) -> PathBuf {
        let scratch = std::env::temp_dir().join(format!(
            "bir-rules-codegen-projections-scratch-{}-{}",
            std::process::id(),
            SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&scratch).expect("create scratch corpus root");

        for relative in COPIED_INPUTS.iter().copied().chain([CORE_ADAPTER_PATH]) {
            copy_into(repo_root, &scratch, Path::new(relative));
        }
        for source in tracked_fixture_paths(repo_root) {
            let relative = source
                .strip_prefix(repo_root)
                .expect("fixture is under the repository root");
            copy_into(repo_root, &scratch, relative);
        }
        scratch
    }

    fn copy_into(repo_root: &Path, scratch: &Path, relative: &Path) {
        let target = scratch.join(relative);
        fs::create_dir_all(target.parent().expect("a copied file has a parent"))
            .expect("create scratch parent");
        fs::copy(repo_root.join(relative), &target).expect("copy scratch input");
    }

    fn tracked_fixture_paths(repo_root: &Path) -> Vec<PathBuf> {
        fixture_paths(&repo_root.join(FIXTURES_PATH)).expect("enumerate tracked fixtures")
    }

    /// The success oracle for the PowerShell port: re-projecting a scratch copy
    /// of the tracked corpus must reproduce the rule set and all 121 fixtures
    /// semantically. This is the Rust form of
    /// `diff <(jq -S -c . tracked) <(jq -S -c . projected)`.
    ///
    /// Byte equality is deliberately not asserted: the tracked files carry
    /// Windows PowerShell 5.1's JSON formatting and key insertion order.
    #[test]
    fn static_projection_reproduces_the_tracked_corpus() {
        let repo_root = landed_repo_root();
        let scratch = materialize_scratch_corpus(&repo_root);

        let report = project_2550q_static_surface(&ProjectStaticSurfaceOptions::new(&scratch))
            .expect("project the scratch corpus");
        assert_eq!(report.executable_raw_field_count, 34);
        assert_eq!(report.documented_only_control_count, 53);
        assert_eq!(report.total_field_count, 94);
        assert_eq!(report.fixture_count, 121);

        let mut relatives = vec![PathBuf::from(RULE_SET_PATH)];
        for fixture in tracked_fixture_paths(&repo_root) {
            relatives.push(
                fixture
                    .strip_prefix(&repo_root)
                    .expect("fixture is under the repository root")
                    .to_path_buf(),
            );
        }
        assert_eq!(relatives.len(), 122);

        for relative in &relatives {
            let tracked = read_json(&repo_root.join(relative)).expect("tracked document parses");
            let projected = read_json(&scratch.join(relative)).expect("projected document parses");
            assert_eq!(
                String::from_utf8(canonical_bytes(&projected)).expect("canonical JSON is UTF-8"),
                String::from_utf8(canonical_bytes(&tracked)).expect("canonical JSON is UTF-8"),
                "{}",
                relative.display()
            );
        }

        fs::remove_dir_all(&scratch).expect("remove scratch corpus");
    }

    /// `[StringComparer]::Ordinal` puts every uppercase ASCII letter before
    /// every lowercase one. PowerShell's default `Sort-Object` does not, and
    /// neither does any locale-aware comparison — either would reorder every
    /// rewritten fixture's field lists.
    #[test]
    fn emitted_identity_order_is_ordinal() {
        let mut identities: BTreeMap<String, ()> = BTreeMap::new();
        for identity in [
            "frm2550qv2024:txtEmail",
            "frm2550qv2024:addInputVat",
            "frm2550qv2024:OptQuarter1",
            "driveSelectTPExport",
            "Zeta",
        ] {
            identities.insert(identity.to_owned(), ());
        }

        assert_eq!(
            identities.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "Zeta",
                "driveSelectTPExport",
                "frm2550qv2024:OptQuarter1",
                "frm2550qv2024:addInputVat",
                "frm2550qv2024:txtEmail",
            ]
        );
    }
}
