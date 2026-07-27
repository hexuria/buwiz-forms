//! Deterministic rebuild of the value-free 2550Q serialization binding
//! inventory.
//!
//! This replaces `rules/tools/build-2550q-serialization-bindings.ps1`, which
//! could only ever run under Windows PowerShell, for two independent reasons:
//!
//! * It joined all six input paths with a literal `\` separator, so on any
//!   other platform every input resolved to a single non-existent file name.
//!   Every path here is resolved through [`crate::path`], which rejects `\`
//!   outright, so that defect class cannot recur.
//! * It appended `[Environment]::NewLine` to the emitted document, so the
//!   artifact's bytes depended on the operating system that produced it. The
//!   tracked inventory is consequently CRLF. This module always emits LF with
//!   exactly one trailing newline, UTF-8 without a BOM.
//!
//! The port is a fidelity port: every count, assertion, literal and emitted
//! value is transcribed unchanged, and `bindings_rebuild_matches_the_tracked_inventory`
//! proves the rebuilt document is semantically identical to the tracked one.
//! Byte equality is deliberately *not* claimed — the tracked file carries
//! Windows PowerShell 5.1's JSON formatting and CRLF line endings.
//!
//! One deliberate behavioural narrowing: PowerShell compares strings
//! case-insensitively by default (`-in`, `-match`, and hashtable lookups all
//! ignore case), which this port does not reproduce; identifier matching here
//! is ordinal. That difference is inert on the pinned corpus — no control id
//! collides case-insensitively, no key resolves to a different family prefix,
//! and every input is SHA-256 pinned, so a corpus that could expose the
//! difference fails the pin check before reaching any comparison.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Number;

use crate::error::{CodegenError, Result};
use crate::files::read_tracked_bytes;
use crate::hash::sha256_hex;
use crate::json::{JsonValue, parse_strict};
use crate::path::{canonical_repo_root, resolve_existing_under, resolve_output_under};

pub const DEFAULT_BINDING_INVENTORY_PATH: &str =
    "rules/forms/2550q-v2024/fixtures/serialization-binding-inventory-v796.json";

const PLAINTEXT_AUDIT_PATH: &str =
    "rules/forms/2550q-v2024/fixtures/plaintext-field-audit-v796.json";
const ENCRYPTED_AUDIT_PATH: &str =
    "rules/forms/2550q-v2024/fixtures/encrypted-field-audit-v796.json";
const CONTROL_INVENTORY_PATH: &str =
    "rules/forms/2550q-v2024/fixtures/runtime-control-inventory-v796.json";
const FAMILY_INVENTORY_PATH: &str =
    "rules/forms/2550q-v2024/fixtures/schedule-family-inventory-v796.json";
const FIELDS_PATH: &str = "rules/forms/2550q-v2024/fields.json";
const RULE_SET_PATH: &str = "rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json";

/// Pinned digests of the five inputs whose content the inventory reproduces.
///
/// These hash the CRLF working tree, which is what the tracked inventory
/// records. `rule_set` is deliberately *not* pinned: the PowerShell original
/// read it without pinning it, and a fidelity port does not add a pin.
const PLAINTEXT_AUDIT_SHA256: &str =
    "cdad131dc0b68fe7c1aca1a82f9d1ad57bffc9988b7aaf5bdca8ac30dd76412e";
const ENCRYPTED_AUDIT_SHA256: &str =
    "ee9355685b7f30b8344299c3139e67c760fda3e7656ba3928c7873b8f6d7fa88";
const CONTROL_INVENTORY_SHA256: &str =
    "8b0740b40f1d68800bacff4f0d6e02cddddc3c63174f107111bc21d6772b760f";
const FAMILY_INVENTORY_SHA256: &str =
    "e4e8bb77fdec565791160b9b6065d1bfee93954eda0c8cce5539db012c9b3815";
const FIELDS_SHA256: &str = "1f19fc553c891a83ca26f38b8228efb92ce90c208e10b1ed6d9b729318c6cedb";

/// The published occurrence decomposition. Never adjust these to make a run
/// pass; they are quoted as fact in the 2550Q review documents.
const PLAINTEXT_OCCURRENCES: usize = 160;
const ENCRYPTED_OCCURRENCES: usize = 159;
const FIELD_BOUND_OCCURRENCES: usize = 159;
const CONTEXT_BOUND_OCCURRENCES: usize = 1;

/// One serialization group: a dynamic row builder in the official runtime and
/// the ordered key families it emits.
struct GroupSpec {
    group_id: &'static str,
    source_lines: &'static str,
    observed_indices: &'static [u64],
    insert_after_key: Option<&'static str>,
    families: &'static [&'static str],
}

const GROUP_SPECS: [GroupSpec; 7] = [
    GroupSpec {
        group_id: "schedule-1-capital-good-row",
        source_lines: "6089-6217",
        observed_indices: &[0, 1],
        insert_after_key: None,
        families: &[
            "txtDatePurchase1",
            "txtSourceCode1",
            "txtDescription1",
            "txtAmountPurchase1",
            "txtInputTax1",
            "txtEstimatedLife1",
            "txtRecognizedLife1",
            "txtAllowedInputTax1",
            "txtBalanceInputTax1",
        ],
    },
    GroupSpec {
        group_id: "schedule-3-creditable-vat-row",
        source_lines: "6390-6473",
        observed_indices: &[0, 1],
        insert_after_key: None,
        families: &[
            "txtDateCovered3",
            "txtDateCovered3To",
            "txtNameWithHoldingAgent3",
            "txtIncomePayment3",
            "txtTotalTaxWithHeld3",
        ],
    },
    GroupSpec {
        group_id: "schedule-4-advance-vat-row",
        source_lines: "6618-6703",
        observed_indices: &[0, 1],
        insert_after_key: None,
        families: &[
            "txtDate4",
            "txtDate4To",
            "txtNameOfMiller4",
            "txtNameOfTaxpayer4",
            "txtOfficialReceiptNumber4",
            "txtAmountPaid4",
        ],
    },
    GroupSpec {
        group_id: "item-19-additional-row",
        source_lines: "6789-7104",
        observed_indices: &[],
        insert_after_key: Some("resultOtherCreditsNo19"),
        families: &[
            "frm2550qv2024:totalTaxPayableNo19Description",
            "frm2550qv2024:totalTaxPayableNo19Amount",
        ],
    },
    GroupSpec {
        group_id: "item-42-additional-row",
        source_lines: "7213-7529",
        observed_indices: &[],
        insert_after_key: Some("resultOtherCreditsNo42"),
        families: &[
            "frm2550qv2024:totalTaxPayableNo42Description",
            "frm2550qv2024:totalTaxPayableNo42Amount",
        ],
    },
    GroupSpec {
        group_id: "item-47-additional-row",
        source_lines: "7609-7907",
        observed_indices: &[],
        insert_after_key: Some("resultOtherCreditsNo47"),
        families: &[
            "frm2550qv2024:totalTaxPayableNo47Description",
            "frm2550qv2024:totalTaxPayableNo47Amount",
        ],
    },
    GroupSpec {
        group_id: "item-56-additional-row",
        source_lines: "7991-8283",
        observed_indices: &[],
        insert_after_key: Some("resultOtherCreditsNo56"),
        families: &[
            "frm2550qv2024:totalTaxPayableNo56Description",
            "frm2550qv2024:totalTaxPayableNo56Amount",
        ],
    },
];

/// The two controls whose plaintext body the official runtime escapes as a
/// JavaScript string literal rather than writing raw.
const JAVASCRIPT_ESCAPED_KEYS: [&str; 2] = [
    "frm2550qv2024:taxpayerName",
    "frm2550qv2024:taxpayerAddress",
];

const ARTIFACT_BOUNDARY_MARKER_EDITABLE: &str = "All Rights Reserved BIR 2012.";
const ARTIFACT_BOUNDARY_MARKER_FINALIZED: &str = "All Rights Reserved BIR 2012.0";
const ENCRYPTION_PROVIDER_SHA256: &str =
    "429337f44f84b93cd1095df48c8f3265e5ede7c646d1b48d9b80f4f92de74d2c";

const UNRESOLVED_EXECUTION_BOUNDARIES: [&str; 7] = [
    "Forty-four derived/alias controls and nine workflow/credential/UI-state controls remain documented-only rather than executable value projections.",
    "Executable serialization still lacks a reviewed mapping from assigned stable instance order to official live DOM/display order.",
    "The runtime separator, text encoding, newline conversion, and non-ASCII behavior are not byte-bound.",
    "The dateFiled context projection is reviewed, but its production clock and timezone provider remains unresolved.",
    "Filename construction, sanitization, overwrite behavior, path confinement, and custody remain unresolved.",
    "The external encryption container, success criteria, failure handling, and determinism remain unresolved.",
    "No filing-safe branch or reviewed production registry entry exists.",
];

#[derive(Clone, Debug)]
pub struct BuildBindingsOptions {
    pub repo_root: PathBuf,
    /// Repository-relative output path. Overridable so a rebuild can be
    /// compared against the tracked artifact without overwriting it.
    pub output_path: String,
}

impl BuildBindingsOptions {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            output_path: DEFAULT_BINDING_INVENTORY_PATH.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BindingsReport {
    pub output_path: PathBuf,
    /// Digest of the bytes this run wrote, not of the tracked artifact.
    pub sha256: String,
    pub plaintext_occurrence_count: usize,
    pub encrypted_occurrence_count: usize,
    pub candidate_v2_bound_occurrence_count: usize,
    pub dynamic_group_count: usize,
}

/// Rebuilds the value-free 2550Q serialization binding inventory and writes it.
/// Deliberately **outside** `rules/`.
///
/// This table is a derived report, not evidence: it is not a declared source,
/// carries no `$schema`, and adds no fact the inventory does not already state.
/// Writing it into the corpus moved the published census from 659 JSON files to
/// 660 — a figure quoted in ten documents, several of them historical records
/// that must not be rewritten. Those eight counts are the load-bearing
/// invariant of `validate-v1`; keeping them genuinely invariant matters more
/// than filing this next to the inventory it reconciles.
pub const DEFAULT_RECONCILIATION_PATH: &str =
    "docs/validation-rules/generated/static-occurrence-reconciliation-v796.json";

pub fn build_2550q_bindings(options: &BuildBindingsOptions) -> Result<BindingsReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let document = build_binding_inventory(&repo_root)?;
    let bytes = render(&document)?;

    // One generated reconciliation of the occurrence partitions, so the four
    // overlapping hand-maintained versions spread across five review documents
    // stop being the only place these numbers agree.
    let reconciliation = build_reconciliation(&document)?;
    let reconciliation_bytes = render(&reconciliation)?;
    let reconciliation_path = resolve_output_under(
        &repo_root,
        DEFAULT_RECONCILIATION_PATH,
        "2550Q static occurrence reconciliation output",
    )?;
    write_atomically(&reconciliation_path, &reconciliation_bytes)?;

    let output_path = resolve_output_under(
        &repo_root,
        &options.output_path,
        "2550Q serialization binding inventory output",
    )?;
    write_atomically(&output_path, &bytes)?;

    Ok(BindingsReport {
        output_path,
        sha256: sha256_hex(&bytes),
        plaintext_occurrence_count: usize_at(
            &document,
            &["observed_contract", "plaintext_occurrence_count"],
        ),
        encrypted_occurrence_count: usize_at(
            &document,
            &["observed_contract", "encrypted_pseudo_div_occurrence_count"],
        ),
        candidate_v2_bound_occurrence_count: usize_at(
            &document,
            &["observed_contract", "candidate_v2_bound_occurrence_count"],
        ),
        dynamic_group_count: GROUP_SPECS.len(),
    })
}

/// Reconciles every published partition of the 160 occurrences against the
/// classifications the inventory now carries, and fails if the arithmetic does
/// not close. Each partition is a different slice of the same population; they
/// were previously maintained by hand in five documents, where nothing checked
/// that they still agreed with one another.
fn build_reconciliation(inventory: &JsonValue) -> Result<JsonValue> {
    let bindings = inventory
        .object()
        .and_then(|inventory| inventory.get("occurrence_bindings"))
        .and_then(|bindings| match bindings {
            JsonValue::Array(values) => Some(values),
            _ => None,
        })
        .ok_or_else(|| CodegenError::new("inventory has no occurrence_bindings"))?;

    let mut by_kind: BTreeMap<&str, usize> = BTreeMap::new();
    let mut by_classification: BTreeMap<&str, usize> = BTreeMap::new();
    for binding in bindings {
        let kind = binding
            .object()
            .and_then(|binding| binding.get("source_projection"))
            .and_then(|projection| projection.object())
            .and_then(|projection| projection.get("kind"))
            .and_then(JsonValue::as_str)
            .unwrap_or("<missing>");
        *by_kind.entry(kind).or_default() += 1;
        let classification = binding
            .object()
            .and_then(|binding| binding.get("classification"))
            .and_then(JsonValue::as_str)
            .unwrap_or("<missing>");
        *by_classification.entry(classification).or_default() += 1;
    }

    let count = |key: &str| by_classification.get(key).copied().unwrap_or_default();
    let executable_singleton = count("executable-singleton");
    let derived_or_alias = count("documented-only-derived-or-alias");
    let workflow_or_ui = count("documented-only-workflow-or-ui");
    let credential = count("documented-only-credential");
    let documented_only = derived_or_alias + workflow_or_ui + credential;
    let static_total = by_kind
        .get("raw-static-control")
        .copied()
        .unwrap_or_default();

    // The reconciliation is only worth generating if it is checked.
    if executable_singleton + documented_only != static_total {
        return Err(CodegenError::new(format!(
            "static occurrence partition does not close: {executable_singleton} executable + \
             {documented_only} documented-only != {static_total} static occurrences"
        )));
    }
    if bindings.len() != by_kind.values().sum::<usize>() {
        return Err(CodegenError::new(
            "occurrence kinds do not sum to the total",
        ));
    }

    let partition = |name: &str, terms: Vec<(&str, usize)>, total: usize| {
        object([
            ("partition", string(name.to_owned())),
            (
                "terms",
                JsonValue::Object(
                    terms
                        .into_iter()
                        .map(|(key, value)| (key.to_owned(), number(value as u64)))
                        .collect(),
                ),
            ),
            ("total", number(total as u64)),
        ])
    };

    Ok(object([
        ("schema_version", string("1.0.0")),
        ("form_id", string("2550q-v2024")),
        ("package_version", string("7.9.6.0")),
        (
            "generated_from",
            string("serialization-binding-inventory-v796.json occurrence classifications"),
        ),
        ("values_emitted", JsonValue::Bool(false)),
        ("occurrence_total", number(bindings.len() as u64)),
        (
            "partitions",
            JsonValue::Array(vec![
                partition(
                    "occurrence-projection-kind",
                    by_kind.iter().map(|(k, v)| (*k, *v)).collect(),
                    bindings.len(),
                ),
                partition(
                    "static-executable-vs-documented-only",
                    vec![
                        ("executable-singleton", executable_singleton),
                        ("documented-only", documented_only),
                    ],
                    static_total,
                ),
                partition(
                    "documented-only-detail",
                    vec![
                        ("derived-or-alias", derived_or_alias),
                        ("workflow-or-ui", workflow_or_ui),
                        ("credential", credential),
                    ],
                    documented_only,
                ),
                partition(
                    "classification",
                    by_classification.iter().map(|(k, v)| (*k, *v)).collect(),
                    bindings.len(),
                ),
            ]),
        ),
    ]))
}

/// Builds the inventory document without touching the output path.
fn build_binding_inventory(repo_root: &Path) -> Result<JsonValue> {
    let plaintext_path = resolve_input(repo_root, PLAINTEXT_AUDIT_PATH)?;
    let encrypted_path = resolve_input(repo_root, ENCRYPTED_AUDIT_PATH)?;
    let controls_path = resolve_input(repo_root, CONTROL_INVENTORY_PATH)?;
    let families_path = resolve_input(repo_root, FAMILY_INVENTORY_PATH)?;
    let fields_path = resolve_input(repo_root, FIELDS_PATH)?;
    let rule_set_path = resolve_input(repo_root, RULE_SET_PATH)?;

    let plaintext = read_pinned(&plaintext_path, "plaintext", PLAINTEXT_AUDIT_SHA256)?;
    let encrypted = read_pinned(&encrypted_path, "encrypted", ENCRYPTED_AUDIT_SHA256)?;
    let controls = read_pinned(&controls_path, "controls", CONTROL_INVENTORY_SHA256)?;
    let families = read_pinned(&families_path, "families", FAMILY_INVENTORY_SHA256)?;
    // The PowerShell original pinned `fields.json`, published its hash in the
    // artifact, and then never read the parsed document — so the inventory
    // attested an input it did not use. It is now load-bearing: every static
    // occurrence must name a key the v1 field corpus actually declares, which
    // is what makes publishing that hash mean something.
    let fields = read_pinned(&fields_path, "fields", FIELDS_SHA256)?;
    let v1_field_keys: BTreeSet<&str> = fields
        .object()
        .and_then(|fields| fields.get("fields"))
        .and_then(|fields| match fields {
            JsonValue::Array(values) => Some(values),
            _ => None,
        })
        .ok_or_else(|| CodegenError::new("fields.json has no fields array"))?
        .iter()
        .filter_map(|field| field.object()?.get("field_key")?.as_str())
        .collect();
    let rule_set = read_json(&rule_set_path)?;

    let plain_keys = string_array(&plaintext, "keys", PLAINTEXT_AUDIT_PATH)?;
    let encrypted_keys = string_array(&encrypted, "keys", ENCRYPTED_AUDIT_PATH)?;
    if plain_keys.len() != PLAINTEXT_OCCURRENCES || encrypted_keys.len() != ENCRYPTED_OCCURRENCES {
        return Err(CodegenError::new("Pinned 2550Q occurrence counts changed."));
    }
    if plain_keys[..ENCRYPTED_OCCURRENCES] != encrypted_keys[..] {
        return Err(CodegenError::new(
            "Encrypted occurrence order is no longer the exact plaintext prefix.",
        ));
    }
    if plain_keys[PLAINTEXT_OCCURRENCES - 1] != "dateFiled" {
        return Err(CodegenError::new(
            "Expected generated plaintext dateFiled as occurrence 160.",
        ));
    }
    if bool_at(&plaintext, "values_emitted") != Some(false)
        || bool_at(&encrypted, "values_emitted") != Some(false)
    {
        return Err(CodegenError::new(
            "Serialization binding inputs must remain value-free.",
        ));
    }

    let control_by_id = index_controls(&controls)?;
    for key in &encrypted_keys {
        if !control_by_id.contains_key(key.as_str()) {
            return Err(CodegenError::new(format!(
                "Observed occurrence does not resolve to one static source control: {key}"
            )));
        }
    }

    let v2_field_ids = rule_set_field_ids(&rule_set)?;
    let prefix_bindings = prefix_bindings(&families)?;

    let occurrences =
        build_occurrences(&plain_keys, &control_by_id, &v2_field_ids, &prefix_bindings)?;

    // Guard the executable/documented-only join. Hashing `rule-set.json` itself
    // would be circular — the inventory is one of its declared sources, so the
    // inventory's own hash feeds back into the file it would be pinning. The
    // field-id surface is the part of the rule set this inventory actually
    // depends on, and it is stable across source-set digest rolls.
    let field_ids_digest = {
        let joined = v2_field_ids.iter().copied().collect::<Vec<_>>().join("\n");
        sha256_hex(joined.as_bytes())
    };

    let mut classification_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for occurrence in &occurrences {
        *classification_counts
            .entry(occurrence.classification)
            .or_default() += 1;
    }
    let actual: Vec<(&str, usize)> = classification_counts
        .iter()
        .map(|(kind, count)| (*kind, *count))
        .collect();
    if actual != EXPECTED_CLASSIFICATION_COUNTS {
        return Err(CodegenError::new(format!(
            "occurrence classification drifted: expected {EXPECTED_CLASSIFICATION_COUNTS:?}, found {actual:?}"
        )));
    }
    let unknown: Vec<&str> = occurrences
        .iter()
        .filter(|occurrence| matches!(occurrence.projection, Projection::StaticControl))
        .map(|occurrence| occurrence.key.as_str())
        .filter(|key| !v1_field_keys.contains(key))
        .collect();
    if !unknown.is_empty() {
        return Err(CodegenError::new(format!(
            "static occurrence(s) name keys absent from the pinned v1 field corpus: {}",
            unknown.join(", ")
        )));
    }

    let group_contracts = build_group_contracts(&occurrences)?;

    let field_bound_count = occurrences
        .iter()
        .filter(|occurrence| {
            occurrence
                .candidate_v2_field_id
                .as_deref()
                .is_some_and(|value| !value.is_empty())
        })
        .count();
    if field_bound_count != FIELD_BOUND_OCCURRENCES {
        return Err(CodegenError::new(format!(
            "Expected the current candidate to identify all 159 live observed controls; got {field_bound_count}."
        )));
    }
    let context_bound_count = occurrences
        .iter()
        .filter(|occurrence| matches!(occurrence.projection, Projection::GeneratedLocalDate))
        .count();
    if context_bound_count != CONTEXT_BOUND_OCCURRENCES {
        return Err(CodegenError::new(format!(
            "Expected generated dateFiled to have one v2 context projection; got {context_bound_count}."
        )));
    }
    let modeled_count = field_bound_count + context_bound_count;

    Ok(object([
        ("schema_version", string("1.0.0")),
        ("form_id", string("2550q-v2024")),
        ("package_version", string("7.9.6.0")),
        ("status", string("documented-only")),
        ("values_emitted", JsonValue::Bool(false)),
        (
            "input_sha256",
            object([
                ("plaintext_field_audit", string(PLAINTEXT_AUDIT_SHA256)),
                ("encrypted_field_audit", string(ENCRYPTED_AUDIT_SHA256)),
                (
                    "runtime_control_inventory",
                    string(CONTROL_INVENTORY_SHA256),
                ),
                ("schedule_family_inventory", string(FAMILY_INVENTORY_SHA256)),
                ("fields", string(FIELDS_SHA256)),
                ("rule_set_field_ids", string(field_ids_digest.clone())),
            ]),
        ),
        (
            "observed_contract",
            object([
                (
                    "plaintext_occurrence_count",
                    number(plain_keys.len() as u64),
                ),
                (
                    "encrypted_pseudo_div_occurrence_count",
                    number(encrypted_keys.len() as u64),
                ),
                (
                    "candidate_v2_bound_occurrence_count",
                    number(modeled_count as u64),
                ),
                (
                    "plaintext_ordered_inventory_sha256",
                    string(required_string(
                        &plaintext,
                        "ordered_field_inventory_sha256",
                        PLAINTEXT_AUDIT_PATH,
                    )?),
                ),
                (
                    "encrypted_ordered_inventory_sha256",
                    string(required_string(
                        &encrypted,
                        "ordered_field_inventory_sha256",
                        ENCRYPTED_AUDIT_PATH,
                    )?),
                ),
                ("encrypted_is_exact_plaintext_prefix", JsonValue::Bool(true)),
                ("plaintext_only_suffix", string("dateFiled")),
            ]),
        ),
        (
            "occurrence_bindings",
            JsonValue::Array(occurrences.iter().map(Occurrence::to_json).collect()),
        ),
        ("dynamic_groups", JsonValue::Array(group_contracts)),
        ("artifact_boundaries", artifact_boundaries()),
        (
            "unresolved_execution_boundaries",
            JsonValue::Array(
                UNRESOLVED_EXECUTION_BOUNDARIES
                    .iter()
                    .map(|boundary| string(*boundary))
                    .collect(),
            ),
        ),
    ]))
}

/// How an occurrence's value is projected from the official runtime.
enum Projection {
    /// The generated `dateFiled` metadata occurrence.
    GeneratedLocalDate,
    /// A singleton control, addressed by its own id.
    StaticControl,
    /// A member of a dynamic row family, addressed by prefix plus legacy index.
    GroupField {
        group_id: &'static str,
        family_ordinal: u64,
        family_prefix: &'static str,
        observed_index: u64,
        observed_legacy_index_key: String,
    },
}

/// Controls the candidate must never capture into the tax draft.
/// `v2-candidate-static-surface-projection-review.md:62-70` enumerates these
/// with the other workflow/UI controls and states the constraint explicitly.
const CREDENTIAL_KEYS: [&str; 4] = [
    "ebirOnlineConfirmUsername",
    "ebirOnlineSecret",
    "ebirOnlineUsername",
    "txtEmail",
];

/// Package workflow and navigation state, from the same enumeration. Finality
/// must not be inferred from `txtFinalFlag`, nor filing data from page state.
const WORKFLOW_OR_UI_KEYS: [&str; 5] = [
    "driveSelectTPExport",
    "frm2550qv2024:txtCurrentPage",
    "frm2550qv2024:txtMaxPage",
    "txtEnroll",
    "txtFinalFlag",
];

/// The published decomposition of the 160 occurrences. Asserted after the build
/// so a silent drift in either the field surface or the control inventory fails
/// the builder instead of quietly re-partitioning "the 53".
const EXPECTED_CLASSIFICATION_COUNTS: [(&str, usize); 6] = [
    ("documented-only-credential", 4),
    ("documented-only-derived-or-alias", 44),
    ("documented-only-workflow-or-ui", 5),
    ("executable-group-field", 40),
    ("executable-singleton", 66),
    ("generated-context-metadata", 1),
];

struct Occurrence {
    plaintext_ordinal: u64,
    encrypted_ordinal: Option<u64>,
    key: String,
    projection: Projection,
    control_kind: &'static str,
    raw_value_kind: &'static str,
    candidate_v2_field_id: Option<String>,
    classification: &'static str,
    plaintext_placement: &'static str,
    encrypted_placement: &'static str,
    semantic_format: &'static str,
    plaintext_body_codec: &'static str,
    source_refs: Vec<String>,
}

impl Occurrence {
    fn to_json(&self) -> JsonValue {
        let source_projection = match &self.projection {
            Projection::GeneratedLocalDate => object([
                ("kind", string("generated-local-date")),
                ("pattern", string("yyyy-mm-dd-slash")),
                ("v2_context_value_id", string("local-current-date")),
            ]),
            Projection::StaticControl => object([
                ("kind", string("raw-static-control")),
                ("control_id", string(self.key.clone())),
            ]),
            Projection::GroupField {
                group_id,
                family_ordinal,
                family_prefix,
                observed_index,
                observed_legacy_index_key,
            } => object([
                ("kind", string("raw-group-field")),
                ("group_id", string(*group_id)),
                ("family_ordinal", number(*family_ordinal)),
                ("family_prefix", string(*family_prefix)),
                ("observed_index", number(*observed_index)),
                (
                    "observed_legacy_index_key",
                    string(observed_legacy_index_key.clone()),
                ),
            ]),
        };
        object([
            ("plaintext_ordinal", number(self.plaintext_ordinal)),
            (
                "encrypted_ordinal",
                match self.encrypted_ordinal {
                    Some(ordinal) => number(ordinal),
                    None => JsonValue::Null,
                },
            ),
            ("key", string(self.key.clone())),
            ("source_projection", source_projection),
            ("control_kind", string(self.control_kind)),
            ("raw_value_kind", string(self.raw_value_kind)),
            ("classification", string(self.classification)),
            (
                "candidate_v2_field_id",
                match &self.candidate_v2_field_id {
                    Some(field_id) => string(field_id.clone()),
                    None => JsonValue::Null,
                },
            ),
            (
                "plaintext_save",
                object([
                    ("placement", string(self.plaintext_placement)),
                    ("semantic_format", string(self.semantic_format)),
                    ("body_codec", string(self.plaintext_body_codec)),
                ]),
            ),
            (
                "encrypted_staging",
                object([
                    ("placement", string(self.encrypted_placement)),
                    ("semantic_format", string(self.semantic_format)),
                    ("body_codec", string("raw-literal")),
                ]),
            ),
            (
                "source_refs",
                JsonValue::Array(self.source_refs.iter().map(string).collect()),
            ),
        ])
    }
}

/// One resolvable family prefix, carrying the group it belongs to.
struct PrefixBinding {
    group_id: &'static str,
    family_ordinal: u64,
    prefix: &'static str,
}

/// Builds the prefix table, ordered by prefix length descending so the longest
/// declared family wins a key that several families could otherwise claim.
fn prefix_bindings(families: &JsonValue) -> Result<Vec<PrefixBinding>> {
    let declared = families
        .object()
        .and_then(|families| families.get("families"))
        .and_then(|families| match families {
            JsonValue::Array(values) => Some(values),
            _ => None,
        })
        .ok_or_else(|| {
            CodegenError::new(format!(
                "{FAMILY_INVENTORY_PATH} is missing array `families`"
            ))
        })?;
    let mut declared_prefixes: Vec<&str> = Vec::with_capacity(declared.len());
    for family in declared {
        declared_prefixes.push(required_string(family, "prefix", FAMILY_INVENTORY_PATH)?);
    }

    let mut bindings = Vec::new();
    for group in &GROUP_SPECS {
        for (offset, prefix) in group.families.iter().enumerate() {
            bindings.push(PrefixBinding {
                group_id: group.group_id,
                family_ordinal: offset as u64 + 1,
                prefix,
            });
        }
    }

    let mut declared_sorted = declared_prefixes.clone();
    declared_sorted.sort_unstable();
    let mut planned_sorted: Vec<&str> = bindings.iter().map(|binding| binding.prefix).collect();
    planned_sorted.sort_unstable();
    if declared_sorted != planned_sorted {
        return Err(CodegenError::new(format!(
            "The seven serialization groups do not cover the 28 extracted dynamic families exactly (declared={}, planned={}).",
            declared_prefixes.len(),
            bindings.len()
        )));
    }

    bindings.sort_by(|left, right| right.prefix.len().cmp(&left.prefix.len()));
    Ok(bindings)
}

/// Resolves a serialized key to `<declared family prefix><legacy index>`.
fn resolve_group_binding(bindings: &[PrefixBinding], key: &str) -> Result<Option<Projection>> {
    for binding in bindings {
        let Some(suffix) = key.strip_prefix(binding.prefix) else {
            continue;
        };
        if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let observed_index = suffix.parse::<u64>().map_err(|source| {
            CodegenError::with_source(
                format!("legacy index `{suffix}` in key `{key}` is not representable"),
                source,
            )
        })?;
        return Ok(Some(Projection::GroupField {
            group_id: binding.group_id,
            family_ordinal: binding.family_ordinal,
            family_prefix: binding.prefix,
            observed_index,
            // Built from the matched digits, not the parsed integer, so a
            // zero-padded legacy key keeps its own identity.
            observed_legacy_index_key: format!("legacy-index-{suffix}"),
        }));
    }
    Ok(None)
}

fn build_occurrences(
    plain_keys: &[String],
    control_by_id: &BTreeMap<&str, &JsonValue>,
    v2_field_ids: &BTreeSet<&str>,
    prefix_bindings: &[PrefixBinding],
) -> Result<Vec<Occurrence>> {
    let mut occurrences = Vec::with_capacity(plain_keys.len());
    for (index, key) in plain_keys.iter().enumerate() {
        let plaintext_ordinal = index as u64 + 1;
        if key == "dateFiled" {
            occurrences.push(Occurrence {
                plaintext_ordinal,
                encrypted_ordinal: None,
                key: key.clone(),
                projection: Projection::GeneratedLocalDate,
                control_kind: "generated-metadata",
                raw_value_kind: "generated-local-date",
                candidate_v2_field_id: None,
                classification: "generated-context-metadata",
                plaintext_placement: "pseudo-div",
                encrypted_placement: "standalone-metadata-after-marker",
                semantic_format: "date-yyyy-mm-dd-slash",
                plaintext_body_codec: "raw-literal",
                source_refs: vec![
                    "official-hta-runtime#saveXML:L5620-L5628".to_owned(),
                    "official-hta-runtime#saveEncryptedProfile:L5240-L5267".to_owned(),
                ],
            });
            continue;
        }

        let control = control_by_id.get(key.as_str()).ok_or_else(|| {
            CodegenError::new(format!(
                "Observed occurrence does not resolve to one static source control: {key}"
            ))
        })?;
        let control_kind = control_kind(control, key)?;
        let source_line = required_integer(control, "source_line", CONTROL_INVENTORY_PATH)?;
        let projection = resolve_group_binding(prefix_bindings, key)?;
        let semantic_format = if matches!(control_kind, "radio" | "checkbox") {
            "javascript-boolean"
        } else {
            "raw-text"
        };
        // Every observed static control has an exact candidate identity even
        // when its value projection remains documented-only and is therefore
        // intentionally absent from the executable field surface.
        let candidate_v2_field_id = match &projection {
            Some(Projection::GroupField { family_prefix, .. })
                if v2_field_ids.contains(family_prefix) =>
            {
                (*family_prefix).to_owned()
            }
            _ => key.clone(),
        };
        let plaintext_body_codec = if JAVASCRIPT_ESCAPED_KEYS.contains(&key.as_str()) {
            "legacy-javascript-escape"
        } else {
            "raw-literal"
        };

        // The executable/documented-only split is a set difference against the
        // rule set's field surface. Recording it per occurrence is the whole
        // point: without it "the 53" is only reproducible by rerunning that
        // join, and nothing notices when it drifts.
        let classification = match &projection {
            Some(Projection::GroupField { family_prefix, .. }) => {
                if v2_field_ids.contains(family_prefix) {
                    "executable-group-field"
                } else {
                    "documented-only-derived-or-alias"
                }
            }
            _ => {
                if v2_field_ids.contains(key.as_str()) {
                    "executable-singleton"
                } else if CREDENTIAL_KEYS.contains(&key.as_str()) {
                    "documented-only-credential"
                } else if WORKFLOW_OR_UI_KEYS.contains(&key.as_str()) {
                    "documented-only-workflow-or-ui"
                } else {
                    "documented-only-derived-or-alias"
                }
            }
        };

        occurrences.push(Occurrence {
            plaintext_ordinal,
            encrypted_ordinal: Some(plaintext_ordinal),
            key: key.clone(),
            projection: projection.unwrap_or(Projection::StaticControl),
            control_kind,
            raw_value_kind: semantic_format,
            candidate_v2_field_id: Some(candidate_v2_field_id),
            classification,
            plaintext_placement: "pseudo-div",
            encrypted_placement: "pseudo-div",
            semantic_format,
            plaintext_body_codec,
            source_refs: vec![
                format!("official-hta-runtime#control:L{source_line}"),
                "official-hta-runtime#saveXML:L5516-L5628".to_owned(),
                "official-hta-runtime#saveEncryptedProfile:L5194-L5267".to_owned(),
            ],
        });
    }
    Ok(occurrences)
}

fn build_group_contracts(occurrences: &[Occurrence]) -> Result<Vec<JsonValue>> {
    let mut contracts = Vec::with_capacity(GROUP_SPECS.len());
    for group in &GROUP_SPECS {
        let observed: Vec<(u64, &'static str)> = occurrences
            .iter()
            .filter_map(|occurrence| match &occurrence.projection {
                Projection::GroupField {
                    group_id,
                    family_prefix,
                    observed_index,
                    ..
                } if *group_id == group.group_id => Some((*observed_index, *family_prefix)),
                _ => None,
            })
            .collect();
        let observed_indices: Vec<u64> = observed
            .iter()
            .map(|(index, _)| *index)
            .collect::<BTreeSet<u64>>()
            .into_iter()
            .collect();
        if observed_indices != group.observed_indices {
            return Err(CodegenError::new(format!(
                "Observed instance set changed for group {}.",
                group.group_id
            )));
        }
        for instance in &observed_indices {
            let actual_prefixes: Vec<&str> = observed
                .iter()
                .filter(|(index, _)| index == instance)
                .map(|(_, prefix)| *prefix)
                .collect();
            if actual_prefixes.len() != group.families.len() {
                return Err(CodegenError::new(format!(
                    "Incomplete observed row {instance} for group {}.",
                    group.group_id
                )));
            }
            if actual_prefixes != group.families {
                return Err(CodegenError::new(format!(
                    "Observed child order changed for group {}, row {instance}.",
                    group.group_id
                )));
            }
        }

        contracts.push(object([
            ("group_id", string(group.group_id)),
            (
                "instance_order",
                string("legacy-zero-based-index-ascending"),
            ),
            ("instance_identity", string("assigned-stable-id")),
            (
                "serialization_instance_order_relation",
                string("unresolved"),
            ),
            ("min_occurs", number(0)),
            ("max_occurs", JsonValue::Null),
            (
                "observed_indices",
                JsonValue::Array(
                    observed_indices
                        .iter()
                        .map(|index| number(*index))
                        .collect(),
                ),
            ),
            (
                "insert_after_key",
                match group.insert_after_key {
                    Some(key) => string(key),
                    None => JsonValue::Null,
                },
            ),
            (
                "families",
                JsonValue::Array(
                    group
                        .families
                        .iter()
                        .enumerate()
                        .map(|(offset, prefix)| {
                            object([
                                ("family_ordinal", number(offset as u64 + 1)),
                                ("key_prefix", string(*prefix)),
                                ("index_base", number(0)),
                                ("index_step", number(1)),
                                ("padding", number(0)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "source_refs",
                JsonValue::Array(vec![
                    string(format!(
                        "official-hta-runtime#dynamic-row-builders:L{}",
                        group.source_lines
                    )),
                    string("v1-schedule-family-inventory#/families"),
                ]),
            ),
        ]));
    }
    Ok(contracts)
}

fn artifact_boundaries() -> JsonValue {
    JsonValue::Array(vec![
        object([
            ("artifact_id", string("official-editable-save")),
            (
                "control_sequence",
                string("occurrence_bindings plaintext order with dynamic groups"),
            ),
            ("marker", string(ARTIFACT_BOUNDARY_MARKER_EDITABLE)),
            (
                "marker_sample_status",
                string("source-established-no-separate-sample"),
            ),
            (
                "date_filed_placement",
                string("final-pseudo-div-before-marker"),
            ),
            ("executable", JsonValue::Bool(false)),
        ]),
        object([
            ("artifact_id", string("official-finalized-save")),
            (
                "control_sequence",
                string("occurrence_bindings plaintext order with dynamic groups"),
            ),
            ("marker", string(ARTIFACT_BOUNDARY_MARKER_FINALIZED)),
            (
                "marker_sample_status",
                string("source-and-sample-established"),
            ),
            (
                "date_filed_placement",
                string("final-pseudo-div-before-marker"),
            ),
            ("executable", JsonValue::Bool(false)),
        ]),
        object([
            ("artifact_id", string("official-encrypted-final-copy")),
            (
                "control_sequence",
                string("first 159 occurrence bindings with dynamic groups"),
            ),
            ("marker", string(ARTIFACT_BOUNDARY_MARKER_FINALIZED)),
            (
                "marker_sample_status",
                string("source-and-decrypted-sample-established"),
            ),
            (
                "date_filed_placement",
                string("standalone-metadata-after-marker"),
            ),
            (
                "encryption_provider_sha256",
                string(ENCRYPTION_PROVIDER_SHA256),
            ),
            ("encryption_contract", string("opaque-unresolved")),
            ("executable", JsonValue::Bool(false)),
        ]),
    ])
}

/// First-wins map from control id to control, matching the PowerShell original:
/// the inventory carries repeated button ids, and only the first occurrence of
/// each is addressable.
fn index_controls(controls: &JsonValue) -> Result<BTreeMap<&str, &JsonValue>> {
    let static_controls = controls
        .object()
        .and_then(|controls| controls.get("static_controls"))
        .and_then(|controls| match controls {
            JsonValue::Array(values) => Some(values),
            _ => None,
        })
        .ok_or_else(|| {
            CodegenError::new(format!(
                "{CONTROL_INVENTORY_PATH} is missing array `static_controls`"
            ))
        })?;

    let mut indexed = BTreeMap::new();
    for control in static_controls {
        let Some(id) = control
            .object()
            .and_then(|control| control.get("id"))
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        indexed.entry(id).or_insert(control);
    }
    Ok(indexed)
}

fn rule_set_field_ids(rule_set: &JsonValue) -> Result<BTreeSet<&str>> {
    let fields = rule_set
        .object()
        .and_then(|rule_set| rule_set.get("fields"))
        .and_then(|fields| match fields {
            JsonValue::Array(values) => Some(values),
            _ => None,
        })
        .ok_or_else(|| CodegenError::new(format!("{RULE_SET_PATH} is missing array `fields`")))?;
    let mut ids = BTreeSet::new();
    for field in fields {
        ids.insert(required_string(field, "field_id", RULE_SET_PATH)?);
    }
    Ok(ids)
}

/// Control kinds are a closed set; returning `'static` strs keeps the emitted
/// vocabulary auditable rather than echoing arbitrary input text.
fn control_kind(control: &JsonValue, key: &str) -> Result<&'static str> {
    let declared = required_string(control, "control_kind", CONTROL_INVENTORY_PATH)?;
    match declared {
        "button" => Ok("button"),
        "checkbox" => Ok("checkbox"),
        "hidden" => Ok("hidden"),
        "password" => Ok("password"),
        "radio" => Ok("radio"),
        "select" => Ok("select"),
        "text" => Ok("text"),
        "textarea" => Ok("textarea"),
        other => Err(CodegenError::new(format!(
            "control `{key}` declares an unknown control_kind `{other}`"
        ))),
    }
}

fn render(document: &JsonValue) -> Result<Vec<u8>> {
    let mut text = serde_json::to_string_pretty(document).map_err(|source| {
        CodegenError::with_source("serialize 2550Q serialization binding inventory", source)
    })?;
    text.push('\n');
    Ok(text.into_bytes())
}

/// Writes through a uniquely named sibling so a failed write can never leave a
/// truncated inventory in place.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| CodegenError::new(format!("output `{}` has no parent", path.display())))?;
    fs::create_dir_all(parent)
        .map_err(|source| CodegenError::io("create output parent", parent, source))?;

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
        "{name}.bir-rules-codegen-bindings-{}",
        std::process::id()
    ));

    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    let mut file = options
        .open(&staging)
        .map_err(|source| CodegenError::io("create staged inventory", &staging, source))?;
    file.write_all(bytes)
        .map_err(|source| CodegenError::io("write staged inventory", &staging, source))?;
    file.sync_all()
        .map_err(|source| CodegenError::io("sync staged inventory", &staging, source))?;
    drop(file);

    fs::rename(&staging, path).map_err(|source| {
        let _ = fs::remove_file(&staging);
        CodegenError::io("install inventory", path, source)
    })
}

fn resolve_input(repo_root: &Path, relative: &str) -> Result<PathBuf> {
    resolve_existing_under(
        repo_root,
        relative,
        &format!("2550Q serialization binding input `{relative}`"),
    )
}

fn read_pinned(path: &Path, name: &str, expected: &str) -> Result<JsonValue> {
    let bytes = read_tracked_bytes(path)?;
    let actual = sha256_hex(&bytes);
    if actual != expected {
        return Err(CodegenError::new(format!(
            "Pinned 2550Q serialization binding input changed: {name} ({actual})"
        )));
    }
    parse_strict(&bytes, path)
}

fn read_json(path: &Path) -> Result<JsonValue> {
    let bytes = read_tracked_bytes(path)?;
    parse_strict(&bytes, path)
}

fn string_array(value: &JsonValue, key: &str, context: &str) -> Result<Vec<String>> {
    let values = value
        .object()
        .and_then(|value| value.get(key))
        .and_then(|value| match value {
            JsonValue::Array(values) => Some(values),
            _ => None,
        })
        .ok_or_else(|| CodegenError::new(format!("{context} is missing array `{key}`")))?;
    values
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| CodegenError::new(format!("{context} `{key}` holds a non-string")))
        })
        .collect()
}

fn required_string<'a>(value: &'a JsonValue, key: &str, context: &str) -> Result<&'a str> {
    value
        .object()
        .and_then(|value| value.get(key))
        .and_then(JsonValue::as_str)
        .ok_or_else(|| CodegenError::new(format!("{context} is missing string `{key}`")))
}

fn required_integer(value: &JsonValue, key: &str, context: &str) -> Result<u64> {
    value
        .object()
        .and_then(|value| value.get(key))
        .and_then(|value| match value {
            JsonValue::Number(number) => number.as_u64(),
            _ => None,
        })
        .ok_or_else(|| CodegenError::new(format!("{context} is missing integer `{key}`")))
}

fn bool_at(value: &JsonValue, key: &str) -> Option<bool> {
    match value.object()?.get(key)? {
        JsonValue::Bool(value) => Some(*value),
        _ => None,
    }
}

/// Reads a count back out of the assembled document so the report can never
/// disagree with the artifact it describes.
fn usize_at(document: &JsonValue, path: &[&str]) -> usize {
    let mut current = document;
    for key in path {
        current = current
            .object()
            .and_then(|object| object.get(*key))
            .expect("the assembled inventory always carries its own counts");
    }
    match current {
        JsonValue::Number(number) => number
            .as_u64()
            .expect("assembled counts are non-negative integers")
            as usize,
        _ => unreachable!("assembled counts are numbers"),
    }
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

fn number(value: u64) -> JsonValue {
    JsonValue::Number(Number::from(value))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        DEFAULT_BINDING_INVENTORY_PATH, GROUP_SPECS, build_binding_inventory, prefix_bindings,
        read_json, render, resolve_input,
    };
    use crate::json::{canonical_bytes, parse_strict};
    use crate::path::canonical_repo_root;

    fn landed_repo_root() -> PathBuf {
        canonical_repo_root(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("repository root")
    }

    /// The success oracle for the PowerShell port: the rebuilt document must be
    /// semantically identical to the tracked inventory. This is the Rust form of
    /// `diff <(jq -S -c . tracked) <(jq -S -c . rebuilt)`.
    ///
    /// Byte equality is deliberately not asserted: the tracked file was written
    /// by Windows PowerShell 5.1 with CRLF line endings.
    #[test]
    fn bindings_rebuild_matches_the_tracked_inventory() {
        let repo_root = landed_repo_root();
        let rebuilt = build_binding_inventory(&repo_root).expect("rebuild binding inventory");

        let tracked_path = resolve_input(&repo_root, DEFAULT_BINDING_INVENTORY_PATH)
            .expect("tracked inventory exists");
        let tracked = read_json(&tracked_path).expect("tracked inventory parses");

        assert_eq!(
            String::from_utf8(canonical_bytes(&rebuilt)).expect("canonical JSON is UTF-8"),
            String::from_utf8(canonical_bytes(&tracked)).expect("canonical JSON is UTF-8"),
        );
    }

    /// The published decomposition, asserted against the rebuilt document so a
    /// silent drift in any input cannot pass unnoticed.
    #[test]
    fn rebuilt_inventory_keeps_the_published_occurrence_decomposition() {
        let rebuilt = build_binding_inventory(&landed_repo_root()).expect("rebuild inventory");
        let contract = rebuilt
            .object()
            .and_then(|document| document.get("observed_contract"))
            .and_then(|contract| contract.object())
            .expect("observed contract");

        for (key, expected) in [
            ("plaintext_occurrence_count", 160u64),
            ("encrypted_pseudo_div_occurrence_count", 159),
            ("candidate_v2_bound_occurrence_count", 160),
        ] {
            let actual = match contract.get(key) {
                Some(crate::json::JsonValue::Number(number)) => number.as_u64(),
                _ => None,
            };
            assert_eq!(actual, Some(expected), "{key}");
        }

        let groups = match rebuilt
            .object()
            .and_then(|document| document.get("dynamic_groups"))
        {
            Some(crate::json::JsonValue::Array(groups)) => groups.len(),
            _ => 0,
        };
        assert_eq!(groups, GROUP_SPECS.len());
        assert_eq!(GROUP_SPECS.len(), 7);
    }

    /// The emitted encoding is the portable one, not the one the PowerShell
    /// produced: LF only, exactly one trailing newline, no BOM.
    #[test]
    fn emitted_encoding_is_lf_only_with_one_trailing_newline() {
        let document = parse_strict(
            br#"{"b":1,"a":[{},[]]}"#,
            std::path::Path::new("encoding.json"),
        )
        .expect("valid JSON");
        let bytes = render(&document).expect("render");
        let text = String::from_utf8(bytes).expect("UTF-8");

        assert!(!text.contains('\r'), "{text}");
        assert!(!text.starts_with('\u{feff}'));
        assert!(text.ends_with("}\n"), "{text}");
        // Sorted keys and two-space indent.
        assert!(text.starts_with("{\n  \"a\": ["), "{text}");
    }

    /// Longest declared family wins, so a key that two families could claim
    /// binds to the more specific one.
    #[test]
    fn prefix_bindings_are_ordered_longest_first() {
        let repo_root = landed_repo_root();
        let families_path = resolve_input(
            &repo_root,
            "rules/forms/2550q-v2024/fixtures/schedule-family-inventory-v796.json",
        )
        .expect("family inventory exists");
        let families = read_json(&families_path).expect("family inventory parses");
        let bindings = prefix_bindings(&families).expect("prefix bindings");

        assert_eq!(bindings.len(), 28);
        for pair in bindings.windows(2) {
            assert!(
                pair[0].prefix.len() >= pair[1].prefix.len(),
                "{} before {}",
                pair[0].prefix,
                pair[1].prefix
            );
        }
    }
}
