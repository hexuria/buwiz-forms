//! Packet-backed, external-only form workspace factory.
//!
//! The factory deliberately emits a `skeleton` snapshot. It mirrors reviewed
//! evidence and exact v1 record identities into a disposable workspace; it
//! never infers executable semantics or writes canonical `rules/`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::error::{CodegenError, Result};
use crate::evidence::{
    DerivedEvidenceKind, EvidenceObservation, EvidenceReviewStatus, RuleSetSourceState,
    VerifiedPacket,
};
use crate::evidence_set::{EVIDENCE_SUMMARY_FORMAT, TRACKED_V1_SOURCE_SET_DOMAIN};
use crate::files::{read_bytes, read_tree};
use crate::hash::{digest_entries, sha256_hex};
use crate::json::{CANONICALIZATION_ID, canonical_bytes, parse_strict};
use crate::path::{portable_join, resolve_existing_under};
use crate::schema::SchemaSet;

const SUMMARY_PATH: &str = "derived/tracked-v1-summary.json";
const HANDOFF_FORMAT: &str = "bir-packet-backed-form-handoff-v1";
const UNRESOLVED_REASON: &str = "The reviewed packet proves this legacy record exists, but no executable v2 semantics have been reviewed.";

#[derive(Debug)]
pub(crate) struct PacketBackedFormPlan {
    pub files: BTreeMap<String, Vec<u8>>,
    pub packet_id: String,
    pub rule_set_id: String,
    pub packet_digest_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RulesIndex {
    forms: Vec<RulesIndexEntry>,
}

#[derive(Clone, Debug, Deserialize)]
struct RulesIndexEntry {
    form_id: String,
    form_code: String,
    revision: String,
    package_version: String,
    path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct TrackedSourceSummary {
    path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug)]
struct TrackedSource {
    path: String,
    canonical_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct InventorySection {
    count: usize,
    records: Vec<InventoryRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct InventoryRecord {
    ordinal: usize,
    record_id: Option<String>,
    json_pointer: String,
    source_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct XmlInventory {
    basis: String,
    projected_field_key_count: usize,
    declared_serializable_count: Option<u64>,
    observed_occurrence_count: usize,
    unresolved_occurrence_count: u64,
    unresolved_count_delta: u64,
    values_emitted: bool,
    records: Vec<XmlRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct XmlRecord {
    ordinal: usize,
    key: String,
    occurrence: Option<usize>,
    observed: bool,
    json_pointer: String,
    source_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordCensus {
    fields: InventorySection,
    validations: InventorySection,
    calculations: InventorySection,
    workflow: InventorySection,
    serialization: InventorySection,
    fixtures: InventorySection,
    explicit_gaps: InventorySection,
    declared_counts: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DerivedSummary {
    format: String,
    canonicalization: String,
    form_id: String,
    tracked_v1_source_set_sha256: String,
    tracked_sources: Vec<TrackedSourceSummary>,
    upstream_assets: Value,
    capture_sessions: Value,
    source_excerpts: Value,
    capture_gaps: Value,
    dom_inventory: InventorySection,
    xml_inventory: XmlInventory,
    runtime_observations: Value,
    save_finalize_reopen: Value,
    census: RecordCensus,
}

struct FormDocuments {
    manifest: Value,
    fields: Value,
    validations: Value,
    calculations: Value,
    workflow: Value,
    negative_cases: Value,
}

struct VerifiedCensus {
    field_records: Vec<InventoryRecord>,
    validation_records: Vec<InventoryRecord>,
    calculation_records: Vec<InventoryRecord>,
    workflow_records: Vec<InventoryRecord>,
    workflow_state_count: usize,
    workflow_transition_count: usize,
    negative_fixture_count: usize,
    concrete_union_fields: u64,
    unbounded_family_members: u64,
    confirmed_official_bugs: u64,
    unverified_gaps: u64,
}

pub(crate) fn build_packet_backed_form_plan(
    repo_root: &Path,
    form_root: &Path,
    form_id: &str,
    packet: &VerifiedPacket,
) -> Result<PacketBackedFormPlan> {
    require_reviewed_packet(packet)?;
    if packet.manifest.form_id != form_id {
        return Err(CodegenError::new(format!(
            "packet form_id `{}` does not match requested form `{form_id}`",
            packet.manifest.form_id
        )));
    }
    if !matches!(
        packet.manifest.rule_set_source_state,
        RuleSetSourceState::Planned { .. }
    ) {
        return Err(CodegenError::new(format!(
            "packet `{}` already pins a v2 source set; packet-backed skeleton staging requires planned/null source state",
            packet.manifest.packet_id
        )));
    }

    let rules_index_path = resolve_existing_under(repo_root, "rules/index.json", "rules index")?;
    let rules_index: RulesIndex = parse_typed_file(&rules_index_path, "rules index")?;
    let entries = rules_index
        .forms
        .iter()
        .filter(|entry| entry.form_id == form_id)
        .collect::<Vec<_>>();
    let [index] = entries.as_slice() else {
        return Err(CodegenError::new(format!(
            "rules/index.json must contain exactly one entry for `{form_id}`, found {}",
            entries.len()
        )));
    };
    let expected_manifest_path = format!("forms/{form_id}/manifest.json");
    if index.path != expected_manifest_path {
        return Err(CodegenError::new(format!(
            "rules index path for `{form_id}` must be `{expected_manifest_path}`, found `{}`",
            index.path
        )));
    }

    let form_tree = read_tree(form_root)?;
    let documents = load_form_documents(form_root, &form_tree)?;
    require_identity(index, &documents.manifest, packet)?;

    let tracked_sources = tracked_v1_sources(form_root)?;
    let tracked_digest = digest_entries(
        TRACKED_V1_SOURCE_SET_DOMAIN,
        tracked_sources
            .iter()
            .map(|source| (source.path.as_str(), source.canonical_bytes.as_slice())),
    );
    if packet.manifest.tracked_v1_source_set_sha256 != tracked_digest {
        return Err(CodegenError::new(format!(
            "tracked v1 source digest mismatch for `{form_id}`: packet={} computed={tracked_digest}",
            packet.manifest.tracked_v1_source_set_sha256
        )));
    }

    let summary = load_summary(packet)?;
    validate_summary_identity(&summary, form_id, &tracked_digest, &tracked_sources)?;
    let census = validate_census(&summary, &documents, form_root)?;

    let classifications = legacy_classifications(&census);
    let sources = legacy_sources(form_id, &form_tree)?;
    let declared_counts = json!({
        "typed_fields": census.field_records.len(),
        "concrete_union_fields": census.concrete_union_fields,
        "field_groups": census.unbounded_family_members,
        "validation_rules": census.validation_records.len(),
        "calculations": census.calculation_records.len(),
        "workflow_states": census.workflow_state_count,
        "workflow_transitions": census.workflow_transition_count,
        "negative_fixtures": census.negative_fixture_count,
        "confirmed_official_bugs": census.confirmed_official_bugs,
        "unverified_gaps": census.unverified_gaps,
    });
    let unresolved_manifest = unresolved_branch(
        "The packet proves this exact form identity, but neither behavior profile has reviewed executable semantics.",
        "v1-manifest",
    );
    let unresolved_policy = unresolved_branch(
        "The packet records legacy rules without a reviewed executable evaluation policy.",
        "v1-validations",
    );
    let rule_set = json!({
        "$schema": "../../../schema/v2/rule-set.schema.json",
        "schema_version": "2.0.0",
        "identity": {
            "rule_set_id": packet.manifest.rule_set_id,
            "form_code": packet.manifest.form_code,
            "form_revision": packet.manifest.form_revision,
            "official_package_version": packet.manifest.official_package_version,
            "source_set_sha256": null
        },
        "review_status": "skeleton",
        "profile_status": {
            "official": unresolved_manifest,
            "filing_safe": unresolved_branch(
                "No filing-safe behavior has been independently reviewed.",
                "v1-manifest",
            )
        },
        "evaluation_policy": {
            "official": unresolved_policy,
            "filing_safe": unresolved_branch(
                "No filing-safe evaluation policy has been independently reviewed.",
                "v1-validations",
            )
        },
        "sources": sources,
        "legacy_v1": {
            "form_id": form_id,
            "schema_version": "1.0.0",
            "mappings": [
                {
                    "artifact": "manifest",
                    "source_id": "v1-manifest",
                    "record_count": 1,
                    "target_sections": ["identity", "sources"],
                    "state": "documented_only"
                },
                {
                    "artifact": "fields",
                    "source_id": "v1-fields",
                    "record_count": census.field_records.len(),
                    "target_sections": ["field-groups", "fields"],
                    "state": "unresolved"
                },
                {
                    "artifact": "validations",
                    "source_id": "v1-validations",
                    "record_count": census.validation_records.len(),
                    "target_sections": ["rules"],
                    "state": "unresolved"
                },
                {
                    "artifact": "calculations",
                    "source_id": "v1-calculations",
                    "record_count": census.calculation_records.len(),
                    "target_sections": ["calculations"],
                    "state": "unresolved"
                },
                {
                    "artifact": "workflow",
                    "source_id": "v1-workflow",
                    "record_count": census.workflow_transition_count,
                    "target_sections": ["workflow"],
                    "state": "unresolved"
                }
            ],
            "record_classifications": classifications,
            "declared_counts": declared_counts
        },
        "context_values": [],
        "field_groups": [],
        "fields": [],
        "evaluation_order": [],
        "calculations": [],
        "rules": [],
        "workflow": unresolved_branch(
            "Every v1 workflow phase and transition is accounted for, but no executable workflow semantics have been reviewed.",
            "v1-workflow",
        ),
        "serialization": {
            "contract_version": "1.0.0",
            "artifacts": []
        },
        "fixtures": []
    });
    let v2_index = json!({
        "$schema": "../../schema/v2/index.schema.json",
        "schema_version": "2.0.0",
        "snapshots": [{
            "rule_set_id": packet.manifest.rule_set_id,
            "form_code": packet.manifest.form_code,
            "form_revision": packet.manifest.form_revision,
            "official_package_version": packet.manifest.official_package_version,
            "source_set_sha256": null,
            "path": format!("{}/rule-set.json", packet.manifest.rule_set_id),
            "review_status": "skeleton",
            "profile_states": {
                "official": "unresolved",
                "filing_safe": "unresolved"
            }
        }]
    });

    let schema_root = resolve_existing_under(repo_root, "rules/schema/v2", "v2 schema root")?;
    let schemas = SchemaSet::load(&schema_root)?;
    validate_generated_json(&schemas, "rule-set.schema.json", &rule_set)?;
    validate_generated_json(&schemas, "index.schema.json", &v2_index)?;

    let handoff = build_handoff(packet, &summary, &census);
    let handoff_md = render_handoff(packet, &summary, &census);
    let mut files = BTreeMap::new();
    for (relative, bytes) in form_tree {
        insert_unique(
            &mut files,
            format!("rules/forms/{form_id}/{relative}"),
            bytes,
        )?;
    }
    for (relative, bytes) in read_tree(&schema_root)? {
        insert_unique(&mut files, format!("rules/schema/v2/{relative}"), bytes)?;
    }
    insert_unique(
        &mut files,
        "rules/ir/v2/index.json".to_owned(),
        canonical_json(&v2_index, "factory v2 index")?,
    )?;
    insert_unique(
        &mut files,
        format!("rules/ir/v2/{}/rule-set.json", packet.manifest.rule_set_id),
        canonical_json(&rule_set, "factory rule set")?,
    )?;
    insert_unique(
        &mut files,
        "HANDOFF.json".to_owned(),
        canonical_json(&handoff, "factory handoff")?,
    )?;
    insert_unique(&mut files, "HANDOFF.md".to_owned(), handoff_md.into_bytes())?;

    Ok(PacketBackedFormPlan {
        files,
        packet_id: packet.manifest.packet_id.clone(),
        rule_set_id: packet.manifest.rule_set_id.clone(),
        packet_digest_sha256: packet.manifest.packet_digest_sha256.clone(),
    })
}

fn require_reviewed_packet(packet: &VerifiedPacket) -> Result<()> {
    if packet.manifest.review.status != EvidenceReviewStatus::Reviewed {
        return Err(CodegenError::new(format!(
            "packet `{}` has review status `{:?}`; packet-backed form staging requires `reviewed`",
            packet.manifest.packet_id, packet.manifest.review.status
        )));
    }
    let unreviewed = packet
        .manifest
        .derived_evidence
        .iter()
        .filter(|file| file.review_status != EvidenceReviewStatus::Reviewed)
        .map(|file| file.path.as_str())
        .collect::<Vec<_>>();
    if !unreviewed.is_empty() {
        return Err(CodegenError::new(format!(
            "packet `{}` has non-reviewed derived evidence: {}",
            packet.manifest.packet_id,
            unreviewed.join(", ")
        )));
    }
    Ok(())
}

fn require_identity(
    index: &RulesIndexEntry,
    manifest: &Value,
    packet: &VerifiedPacket,
) -> Result<()> {
    let identities = [
        (
            "form_id",
            index.form_id.as_str(),
            required_string(manifest, "form_id")?,
        ),
        (
            "form_code",
            index.form_code.as_str(),
            required_string(manifest, "form_code")?,
        ),
        (
            "form_revision",
            index.revision.as_str(),
            required_string(manifest, "revision")?,
        ),
        (
            "official_package_version",
            index.package_version.as_str(),
            required_string(manifest, "package_version")?,
        ),
    ];
    for (label, indexed, v1) in identities {
        if indexed != v1 {
            return Err(CodegenError::new(format!(
                "{label} differs between rules index `{indexed}` and v1 manifest `{v1}`"
            )));
        }
    }
    for (label, expected, actual) in [
        (
            "form_code",
            index.form_code.as_str(),
            packet.manifest.form_code.as_str(),
        ),
        (
            "form_revision",
            index.revision.as_str(),
            packet.manifest.form_revision.as_str(),
        ),
        (
            "official_package_version",
            index.package_version.as_str(),
            packet.manifest.official_package_version.as_str(),
        ),
    ] {
        if expected != actual {
            return Err(CodegenError::new(format!(
                "{label} differs between rules index `{expected}` and packet `{actual}`"
            )));
        }
    }
    if packet.manifest.capture_provenance.official_app_version
        != packet.manifest.official_package_version
    {
        return Err(CodegenError::new(
            "packet capture official_app_version differs from exact official package identity",
        ));
    }
    Ok(())
}

fn load_summary(packet: &VerifiedPacket) -> Result<DerivedSummary> {
    let summaries = packet
        .manifest
        .derived_evidence
        .iter()
        .filter(|file| file.kind == DerivedEvidenceKind::RecordCensus)
        .collect::<Vec<_>>();
    let [declared] = summaries.as_slice() else {
        return Err(CodegenError::new(format!(
            "packet `{}` must contain exactly one record-census file, found {}",
            packet.manifest.packet_id,
            summaries.len()
        )));
    };
    if declared.path != SUMMARY_PATH || declared.observation != EvidenceObservation::Observed {
        return Err(CodegenError::new(format!(
            "packet record census must be observed at `{SUMMARY_PATH}`"
        )));
    }
    let bytes = packet
        .derived_files
        .get(SUMMARY_PATH)
        .expect("verified packet inventory contains every declared derived file");
    let value = parse_strict(bytes, &packet.root.join(SUMMARY_PATH))?.into_serde();
    serde_json::from_value(value).map_err(|source| {
        CodegenError::with_source("load closed packet record-census summary", source)
    })
}

fn validate_summary_identity(
    summary: &DerivedSummary,
    form_id: &str,
    tracked_digest: &str,
    tracked_sources: &[TrackedSource],
) -> Result<()> {
    if summary.format != EVIDENCE_SUMMARY_FORMAT
        || summary.canonicalization != CANONICALIZATION_ID
        || summary.form_id != form_id
        || summary.tracked_v1_source_set_sha256 != tracked_digest
    {
        return Err(CodegenError::new(
            "packet summary format/form/digest identity does not match the staged form",
        ));
    }
    let expected = tracked_sources
        .iter()
        .map(|source| TrackedSourceSummary {
            path: source.path.clone(),
            size_bytes: source.canonical_bytes.len() as u64,
            sha256: sha256_hex(&source.canonical_bytes),
        })
        .collect::<Vec<_>>();
    if summary.tracked_sources != expected {
        return Err(CodegenError::new(
            "packet tracked_sources are not an exact ordered census of the canonical v1 source set",
        ));
    }
    for (label, value) in [
        ("upstream_assets", &summary.upstream_assets),
        ("capture_sessions", &summary.capture_sessions),
        ("source_excerpts", &summary.source_excerpts),
        ("capture_gaps", &summary.capture_gaps),
        ("runtime_observations", &summary.runtime_observations),
        ("save_finalize_reopen", &summary.save_finalize_reopen),
    ] {
        if value.is_null() {
            return Err(CodegenError::new(format!(
                "packet summary `{label}` must be structurally present"
            )));
        }
    }
    Ok(())
}

fn validate_census(
    summary: &DerivedSummary,
    documents: &FormDocuments,
    form_root: &Path,
) -> Result<VerifiedCensus> {
    let fields = inventory_from_array(&documents.fields, "fields", "field_key")?;
    let validations = inventory_from_array(&documents.validations, "rules", "rule_id")?;
    let calculations =
        inventory_from_array(&documents.calculations, "calculations", "calculation_id")?;
    let phases = inventory_from_array(&documents.workflow, "phases", "phase")?;
    let transitions = inventory_from_array(&documents.workflow, "transitions", "action")?;
    let mut workflow = phases.clone();
    for mut record in transitions.clone() {
        record.ordinal = workflow.len() + 1;
        workflow.push(record);
    }
    let fixtures = fixture_inventory(form_root)?;
    let serialization = serialization_inventory(form_root, &fields)?;
    let gap_count = manifest_count(&documents.manifest, "unverified_gaps")?;
    let gaps = (0..gap_count as usize)
        .map(|index| InventoryRecord {
            ordinal: index + 1,
            record_id: None,
            json_pointer: format!("/declared-gaps/{index}"),
            source_refs: vec!["gaps.md".to_owned()],
        })
        .collect::<Vec<_>>();

    require_section("fields", &summary.census.fields, &fields)?;
    require_section("validations", &summary.census.validations, &validations)?;
    require_section("calculations", &summary.census.calculations, &calculations)?;
    require_section("workflow", &summary.census.workflow, &workflow)?;
    require_section(
        "serialization",
        &summary.census.serialization,
        &serialization,
    )?;
    require_section("fixtures", &summary.census.fixtures, &fixtures)?;
    require_section("explicit_gaps", &summary.census.explicit_gaps, &gaps)?;
    require_section("dom_inventory", &summary.dom_inventory, &fields)?;

    for (key, actual) in [
        ("fields", fields.len() as u64),
        ("validations", validations.len() as u64),
        ("calculations", calculations.len() as u64),
        ("unverified_gaps", gap_count),
    ] {
        if summary.census.declared_counts.get(key) != Some(&actual) {
            return Err(CodegenError::new(format!(
                "packet census declared_counts.{key} does not prove exact count {actual}"
            )));
        }
    }
    if summary.census.declared_counts.len() != 4 {
        return Err(CodegenError::new(
            "packet census declared_counts must contain exactly fields, validations, calculations, and unverified_gaps",
        ));
    }
    for (key, actual) in [
        ("typed_fields", fields.len() as u64),
        ("validation_rules", validations.len() as u64),
        ("calculations", calculations.len() as u64),
    ] {
        let declared = manifest_count(&documents.manifest, key)?;
        if declared != actual {
            return Err(CodegenError::new(format!(
                "v1 manifest counts.{key}={declared} differs from exact source count {actual}"
            )));
        }
    }
    validate_xml_inventory(
        &summary.xml_inventory,
        &documents.fields,
        &fields,
        form_root,
    )?;

    let negative_fixture_count = required_array(&documents.negative_cases, "cases")?.len();
    let concrete_union_fields =
        optional_manifest_count(&documents.manifest, "concrete_union_fields")
            .or_else(|| optional_manifest_count(&documents.manifest, "editable_xml_fields"))
            .unwrap_or(fields.len() as u64);
    let unbounded_family_members =
        optional_manifest_count(&documents.manifest, "unbounded_families").unwrap_or(0);
    let confirmed_official_bugs = manifest_count(&documents.manifest, "confirmed_official_bugs")?;
    Ok(VerifiedCensus {
        field_records: fields,
        validation_records: validations,
        calculation_records: calculations,
        workflow_records: workflow,
        workflow_state_count: phases.len(),
        workflow_transition_count: transitions.len(),
        negative_fixture_count,
        concrete_union_fields,
        unbounded_family_members,
        confirmed_official_bugs,
        unverified_gaps: gap_count,
    })
}

fn validate_xml_inventory(
    xml: &XmlInventory,
    fields_document: &Value,
    fields: &[InventoryRecord],
    form_root: &Path,
) -> Result<()> {
    if xml.values_emitted {
        return Err(CodegenError::new(
            "packet xml inventory must remain value-free (`values_emitted: false`)",
        ));
    }
    let binding = form_root.join("fixtures/serialization-binding-inventory-v796.json");
    if binding.exists() {
        let value: Value = parse_typed_file(&binding, "serialization binding inventory")?;
        let bindings = required_array(&value, "occurrence_bindings")?;
        let mut occurrences = BTreeMap::<String, usize>::new();
        let expected_records = bindings
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let object = value.as_object().ok_or_else(|| {
                    CodegenError::new("serialization occurrence bindings must be objects")
                })?;
                let key = object
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        CodegenError::new("serialization occurrence binding is missing `key`")
                    })?
                    .to_owned();
                let occurrence = occurrences.entry(key.clone()).or_insert(0);
                *occurrence += 1;
                Ok(XmlRecord {
                    ordinal: index + 1,
                    key,
                    occurrence: Some(*occurrence),
                    observed: true,
                    json_pointer: format!(
                        "fixtures/serialization-binding-inventory-v796.json#/occurrence_bindings/{index}"
                    ),
                    source_refs: string_source_refs(object)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if xml.basis != "explicit-serialization-binding"
            || xml.projected_field_key_count != fields.len()
            || xml.declared_serializable_count != Some(bindings.len() as u64)
            || xml.observed_occurrence_count != bindings.len()
            || xml.unresolved_occurrence_count != 0
            || xml.unresolved_count_delta != 0
            || xml.records != expected_records
        {
            return Err(CodegenError::new(
                "packet explicit serialization census does not prove exact binding counts",
            ));
        }
    } else {
        let declared = fields_document
            .get("runtime_serializable_element_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                CodegenError::new(
                    "packet-backed staging requires an exact runtime_serializable_element_count",
                )
            })?;
        let projected = fields.len() as u64;
        let expected_records = fields
            .iter()
            .map(|record| XmlRecord {
                ordinal: record.ordinal,
                key: record
                    .record_id
                    .clone()
                    .expect("field census records always carry field_key"),
                occurrence: None,
                observed: false,
                json_pointer: record.json_pointer.clone(),
                source_refs: record.source_refs.clone(),
            })
            .collect::<Vec<_>>();
        if xml.basis != "field-key-projection"
            || xml.projected_field_key_count != fields.len()
            || xml.declared_serializable_count != Some(declared)
            || xml.observed_occurrence_count != 0
            || xml.unresolved_occurrence_count != declared
            || xml.unresolved_count_delta != declared.abs_diff(projected)
            || xml.records != expected_records
        {
            return Err(CodegenError::new(
                "packet projected serialization census does not prove exact declared/projected counts",
            ));
        }
    }
    for (index, record) in xml.records.iter().enumerate() {
        if record.ordinal != index + 1 {
            return Err(CodegenError::new(
                "packet serialization record ordinals must be contiguous from 1",
            ));
        }
    }
    Ok(())
}

fn legacy_classifications(census: &VerifiedCensus) -> Vec<Value> {
    let mut classifications = Vec::new();
    for (artifact, source_id, records, prefix) in [
        (
            "fields",
            "v1-fields",
            census.field_records.as_slice(),
            "#/fields/",
        ),
        (
            "validations",
            "v1-validations",
            census.validation_records.as_slice(),
            "#/rules/",
        ),
        (
            "calculations",
            "v1-calculations",
            census.calculation_records.as_slice(),
            "#/calculations/",
        ),
    ] {
        for (index, record) in records.iter().enumerate() {
            let locator = format!("{prefix}{index}");
            classifications.push(json!({
                "outcome": "unresolved",
                "artifact": artifact,
                "legacy_id": record.record_id,
                "locator": locator,
                "reason": UNRESOLVED_REASON,
                "source_refs": [{"source_id": source_id, "locator": locator}]
            }));
        }
    }
    for record in &census.workflow_records {
        let locator = format!("#{}", record.json_pointer);
        classifications.push(json!({
            "outcome": "unresolved",
            "artifact": "workflow",
            "locator": locator,
            "reason": UNRESOLVED_REASON,
            "source_refs": [{"source_id": "v1-workflow", "locator": locator}]
        }));
    }
    classifications
}

fn build_handoff(
    packet: &VerifiedPacket,
    summary: &DerivedSummary,
    census: &VerifiedCensus,
) -> Value {
    let serialization_records = summary
        .xml_inventory
        .records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            json!({
                "ordinal": record.ordinal,
                "key": record.key,
                "occurrence": record.occurrence,
                "observed": record.observed,
                "state": "unresolved",
                "packet_locator": format!("{SUMMARY_PATH}#/xml_inventory/records/{index}"),
                "source_locator": source_locator(&packet.manifest.form_id, &record.json_pointer),
                "source_refs": record.source_refs
            })
        })
        .collect::<Vec<_>>();
    let unknown_slots = (0..summary.xml_inventory.unresolved_count_delta)
        .map(|offset| {
            json!({
                "slot": offset + 1,
                "state": "unresolved",
                "packet_locator": format!("{SUMMARY_PATH}#/xml_inventory/unresolved_count_delta"),
                "source_locator": null,
                "reason": "The packet proves a count delta but does not identify which occurrence fills this slot."
            })
        })
        .collect::<Vec<_>>();
    json!({
        "format": HANDOFF_FORMAT,
        "canonicalization": CANONICALIZATION_ID,
        "packet": {
            "packet_id": packet.manifest.packet_id,
            "packet_digest_sha256": packet.manifest.packet_digest_sha256,
            "review_status": "reviewed",
            "record_census_path": SUMMARY_PATH
        },
        "identity": {
            "form_id": packet.manifest.form_id,
            "form_code": packet.manifest.form_code,
            "form_revision": packet.manifest.form_revision,
            "official_package_version": packet.manifest.official_package_version,
            "rule_set_id": packet.manifest.rule_set_id,
            "source_set_sha256": null,
            "tracked_v1_source_set_sha256": packet.manifest.tracked_v1_source_set_sha256
        },
        "review_status": "skeleton",
        "canonical_integration_performed": false,
        "proves_executable_semantics": false,
        "legacy_record_census": {
            "fields": census.field_records.len(),
            "validations": census.validation_records.len(),
            "calculations": census.calculation_records.len(),
            "workflow_states": census.workflow_state_count,
            "workflow_transitions": census.workflow_transition_count,
            "classifications": census.field_records.len()
                + census.validation_records.len()
                + census.calculation_records.len()
                + census.workflow_records.len()
        },
        "serialization_occurrences": {
            "blocking_gap": true,
            "state": "unresolved",
            "basis": summary.xml_inventory.basis,
            "values_emitted": false,
            "projected_count": summary.xml_inventory.projected_field_key_count,
            "declared_count": summary.xml_inventory.declared_serializable_count,
            "observed_count": summary.xml_inventory.observed_occurrence_count,
            "unresolved_count": summary.xml_inventory.unresolved_occurrence_count,
            "unresolved_count_delta": summary.xml_inventory.unresolved_count_delta,
            "records": serialization_records,
            "unknown_occurrence_slots": unknown_slots,
            "reason": "The packet does not review artifact identities, targets, value projections, byte order, codecs, or executable nodes."
        },
        "blocking_gaps": [
            "official-profile-unresolved",
            "filing-safe-profile-unresolved",
            "evaluation-policy-unresolved",
            "workflow-semantics-unresolved",
            "serialization-artifact-identity-and-target-unresolved"
        ]
    })
}

fn render_handoff(
    packet: &VerifiedPacket,
    summary: &DerivedSummary,
    census: &VerifiedCensus,
) -> String {
    format!(
        "# Packet-backed form handoff\n\n\
         This is an external `skeleton` workspace for `{}`. No canonical corpus integration, \
         executable semantics, promotion, filing-safe decision, or runtime authority was created.\n\n\
         - Rule-set ID: `{}`\n\
         - Reviewed packet: `{}`\n\
         - Packet digest: `{}`\n\
         - Tracked v1 digest: `{}`\n\
         - Legacy records: {} fields, {} validations, {} calculations, {} workflow phases, {} workflow transitions\n\
         - Serialization: {} projected, {} observed, {} unresolved, count delta {}\n\n\
         `HANDOFF.json` is the deterministic machine handoff. Serialization remains a blocking \
         gap: the packet does not establish artifact identities, targets, value projections, byte \
         order, codecs, or executable nodes. The v2 serialization artifact list is intentionally empty.\n",
        packet.manifest.form_id,
        packet.manifest.rule_set_id,
        packet.manifest.packet_id,
        packet.manifest.packet_digest_sha256,
        packet.manifest.tracked_v1_source_set_sha256,
        census.field_records.len(),
        census.validation_records.len(),
        census.calculation_records.len(),
        census.workflow_state_count,
        census.workflow_transition_count,
        summary.xml_inventory.projected_field_key_count,
        summary.xml_inventory.observed_occurrence_count,
        summary.xml_inventory.unresolved_occurrence_count,
        summary.xml_inventory.unresolved_count_delta,
    )
}

fn legacy_sources(form_id: &str, tree: &BTreeMap<String, Vec<u8>>) -> Result<Vec<Value>> {
    [
        ("v1-manifest", "legacy-v1-manifest", "manifest.json"),
        ("v1-fields", "legacy-v1-fields", "fields.json"),
        (
            "v1-validations",
            "legacy-v1-validations",
            "validations.json",
        ),
        (
            "v1-calculations",
            "legacy-v1-calculations",
            "calculations.json",
        ),
        ("v1-workflow", "legacy-v1-workflow", "workflow.json"),
    ]
    .into_iter()
    .map(|(source_id, kind, relative)| {
        let bytes = tree
            .get(relative)
            .ok_or_else(|| CodegenError::new(format!("v1 form source is missing `{relative}`")))?;
        Ok(json!({
            "source_id": source_id,
            "kind": kind,
            "path": format!("forms/{form_id}/{relative}"),
            "sha256": sha256_hex(bytes)
        }))
    })
    .collect()
}

fn unresolved_branch(reason: &str, source_id: &str) -> Value {
    json!({
        "state": "unresolved",
        "reason": reason,
        "source_refs": [{"source_id": source_id}]
    })
}

fn source_locator(form_id: &str, pointer: &str) -> Option<String> {
    if pointer.starts_with('/') {
        Some(format!("rules/forms/{form_id}/fields.json#{pointer}"))
    } else if pointer.contains("#/") {
        Some(format!("rules/forms/{form_id}/{pointer}"))
    } else {
        None
    }
}

fn require_section(
    label: &str,
    actual: &InventorySection,
    expected: &[InventoryRecord],
) -> Result<()> {
    if actual.count != expected.len() || actual.records != expected {
        return Err(CodegenError::new(format!(
            "packet `{label}` census is not an exact ordered record bijection"
        )));
    }
    Ok(())
}

fn inventory_from_array(document: &Value, key: &str, id_key: &str) -> Result<Vec<InventoryRecord>> {
    required_array(document, key)?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let object = value
                .as_object()
                .ok_or_else(|| CodegenError::new(format!("{key}[{index}] must be an object")))?;
            let record_id = object
                .get(id_key)
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    CodegenError::new(format!("{key}[{index}] is missing string `{id_key}`"))
                })?
                .to_owned();
            Ok(InventoryRecord {
                ordinal: index + 1,
                record_id: Some(record_id),
                json_pointer: format!("/{key}/{index}"),
                source_refs: string_source_refs(object)?,
            })
        })
        .collect()
}

fn serialization_inventory(
    form_root: &Path,
    fields: &[InventoryRecord],
) -> Result<Vec<InventoryRecord>> {
    let path = form_root.join("fixtures/serialization-binding-inventory-v796.json");
    if !path.exists() {
        return Ok(fields.to_vec());
    }
    let value: Value = parse_typed_file(&path, "serialization binding inventory")?;
    required_array(&value, "occurrence_bindings")?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let object = value
                .as_object()
                .ok_or_else(|| CodegenError::new("serialization bindings must be objects"))?;
            let key = object
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| CodegenError::new("serialization binding is missing string `key`"))?;
            Ok(InventoryRecord {
                ordinal: index + 1,
                record_id: Some(key.to_owned()),
                json_pointer: format!(
                    "fixtures/serialization-binding-inventory-v796.json#/occurrence_bindings/{index}"
                ),
                source_refs: string_source_refs(object)?,
            })
        })
        .collect()
}

fn fixture_inventory(form_root: &Path) -> Result<Vec<InventoryRecord>> {
    let fixture_root = form_root.join("fixtures");
    let mut records = Vec::new();
    for (relative, bytes) in read_tree(&fixture_root)? {
        if !relative.ends_with(".json") {
            continue;
        }
        let path = portable_join(&fixture_root, &relative, "fixture census path")?;
        let value = parse_strict(&bytes, &path)?.into_serde();
        let Some(object) = value.as_object() else {
            continue;
        };
        for (key, value) in object {
            let Some(array) = value.as_array() else {
                continue;
            };
            for (index, entry) in array.iter().enumerate() {
                let Some(entry) = entry.as_object() else {
                    continue;
                };
                records.push(InventoryRecord {
                    ordinal: records.len() + 1,
                    record_id: fixture_record_id(entry),
                    json_pointer: format!("{relative}#/{key}/{index}"),
                    source_refs: string_source_refs(entry)?,
                });
            }
        }
    }
    Ok(records)
}

fn fixture_record_id(object: &Map<String, Value>) -> Option<String> {
    [
        "case_id",
        "observation_id",
        "fixture_id",
        "test_id",
        "record_id",
        "asset_id",
        "calculation_id",
        "rule_id",
        "field_id",
        "field_key",
        "transition_id",
        "group_id",
        "source_id",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str).map(str::to_owned))
}

fn string_source_refs(object: &Map<String, Value>) -> Result<Vec<String>> {
    let Some(value) = object.get("source_refs") else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or_else(|| CodegenError::new("v1 source_refs must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| CodegenError::new("v1 source_refs must contain strings"))
        })
        .collect()
}

fn load_form_documents(
    form_root: &Path,
    tree: &BTreeMap<String, Vec<u8>>,
) -> Result<FormDocuments> {
    Ok(FormDocuments {
        manifest: parse_tree_json(form_root, tree, "manifest.json")?,
        fields: parse_tree_json(form_root, tree, "fields.json")?,
        validations: parse_tree_json(form_root, tree, "validations.json")?,
        calculations: parse_tree_json(form_root, tree, "calculations.json")?,
        workflow: parse_tree_json(form_root, tree, "workflow.json")?,
        negative_cases: parse_tree_json(form_root, tree, "fixtures/negative-cases.json")?,
    })
}

fn parse_tree_json(
    form_root: &Path,
    tree: &BTreeMap<String, Vec<u8>>,
    relative: &str,
) -> Result<Value> {
    let bytes = tree
        .get(relative)
        .ok_or_else(|| CodegenError::new(format!("v1 form is missing `{relative}`")))?;
    let path = portable_join(form_root, relative, "v1 source path")?;
    Ok(parse_strict(bytes, &path)?.into_serde())
}

fn tracked_v1_sources(form_root: &Path) -> Result<Vec<TrackedSource>> {
    let mut sources = Vec::new();
    for (path, bytes) in read_tree(form_root)? {
        let name = path.rsplit('/').next().unwrap_or(path.as_str());
        if matches!(name, "README.md" | "HANDOFF.md") || name.starts_with("v2-") {
            continue;
        }
        let canonical_bytes = if path.ends_with(".json") {
            let source_path = portable_join(form_root, &path, "tracked v1 JSON source")?;
            canonical_bytes(&parse_strict(&bytes, &source_path)?)
        } else if path.ends_with(".md") {
            canonical_text(&bytes, &path)?
        } else {
            return Err(CodegenError::new(format!(
                "tracked v1 source `{path}` has unsupported non-text extension"
            )));
        };
        sources.push(TrackedSource {
            path,
            canonical_bytes,
        });
    }
    let paths = sources
        .iter()
        .map(|source| source.path.as_str())
        .collect::<BTreeSet<_>>();
    for required in [
        "manifest.json",
        "fields.json",
        "validations.json",
        "calculations.json",
        "workflow.json",
        "gaps.md",
        "fixtures/negative-cases.json",
    ] {
        if !paths.contains(required) {
            return Err(CodegenError::new(format!(
                "tracked v1 source set is missing required `{required}`"
            )));
        }
    }
    Ok(sources)
}

fn canonical_text(bytes: &[u8], path: &str) -> Result<Vec<u8>> {
    let text = std::str::from_utf8(bytes).map_err(|source| {
        CodegenError::with_source(format!("tracked text source `{path}` is not UTF-8"), source)
    })?;
    Ok(text
        .strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .into_bytes())
}

fn manifest_count(manifest: &Value, key: &str) -> Result<u64> {
    optional_manifest_count(manifest, key)
        .ok_or_else(|| CodegenError::new(format!("v1 manifest is missing integer counts.{key}")))
}

fn optional_manifest_count(manifest: &Value, key: &str) -> Option<u64> {
    manifest
        .get("counts")
        .and_then(Value::as_object)
        .and_then(|counts| counts.get(key))
        .and_then(Value::as_u64)
}

fn required_array<'a>(document: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    document
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| CodegenError::new(format!("document is missing array `{key}`")))
}

fn required_string<'a>(document: &'a Value, key: &str) -> Result<&'a str> {
    document
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| CodegenError::new(format!("document is missing string `{key}`")))
}

fn parse_typed_file<T: for<'de> Deserialize<'de>>(path: &Path, label: &str) -> Result<T> {
    let bytes = read_bytes(path)?;
    let value = parse_strict(&bytes, path)?.into_serde();
    serde_json::from_value(value)
        .map_err(|source| CodegenError::with_source(format!("load {label}"), source))
}

fn canonical_json(value: &impl Serialize, label: &str) -> Result<Vec<u8>> {
    let ordinary = serde_json::to_vec(value)
        .map_err(|source| CodegenError::with_source(format!("serialize {label}"), source))?;
    Ok(canonical_bytes(&parse_strict(&ordinary, Path::new(label))?))
}

fn validate_generated_json(schemas: &SchemaSet, schema: &str, value: &Value) -> Result<()> {
    let bytes = canonical_json(value, schema)?;
    let parsed = parse_strict(&bytes, Path::new(schema))?;
    schemas.validate(schema, &parsed)
}

fn insert_unique(
    files: &mut BTreeMap<String, Vec<u8>>,
    path: String,
    bytes: Vec<u8>,
) -> Result<()> {
    if files.insert(path.clone(), bytes).is_some() {
        return Err(CodegenError::new(format!(
            "factory output path `{path}` is produced more than once"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::{Value, json};

    use super::{
        SUMMARY_PATH, build_packet_backed_form_plan, canonical_json, sha256_hex, tracked_v1_sources,
    };
    use crate::audit::{AuditOptions, audit};
    use crate::evidence::{
        DerivedEvidenceFile, DerivedEvidenceKind, EVIDENCE_PACKET_FORMAT,
        EvidenceCaptureOperatingSystem, EvidenceCaptureProvenance, EvidenceObservation,
        EvidencePacketManifest, EvidenceReview, EvidenceReviewStatus, RuleSetSourceState,
        StageFormOptions, VerifiedPacket, stage_form,
    };
    use crate::evidence_set::{EVIDENCE_SUMMARY_FORMAT, TRACKED_V1_SOURCE_SET_DOMAIN};
    use crate::files::{read_tree, write_tree_atomically};
    use crate::hash::digest_entries;
    use crate::json::CANONICALIZATION_ID;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct FactoryFixture {
        root: PathBuf,
        repo: PathBuf,
        form_root: PathBuf,
        packet: VerifiedPacket,
    }

    impl Drop for FactoryFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn fixture() -> FactoryFixture {
        let root = std::env::temp_dir().join(format!(
            "bir-form-factory-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let repo = root.join("repo");
        let form_root = repo.join("rules/forms/form-a");
        fs::create_dir_all(form_root.join("fixtures")).expect("create form fixture");
        copy_schema_tree(&repo);

        write_json(
            &form_root.join("manifest.json"),
            &json!({
                "schema_version": "1.0.0",
                "form_id": "form-a",
                "form_code": "1234A",
                "revision": "2026-01-01",
                "package_version": "7.9.6.0",
                "counts": {
                    "typed_fields": 1,
                    "concrete_union_fields": 1,
                    "unbounded_families": 0,
                    "validation_rules": 1,
                    "calculations": 1,
                    "confirmed_official_bugs": 0,
                    "unverified_gaps": 0
                }
            }),
        );
        write_json(
            &form_root.join("fields.json"),
            &json!({
                "runtime_serializable_element_count": 2,
                "fields": [{
                    "field_key": "field-a",
                    "source_refs": ["source#field-a"]
                }]
            }),
        );
        write_json(
            &form_root.join("validations.json"),
            &json!({
                "rules": [{
                    "rule_id": "rule-a",
                    "source_refs": ["source#rule-a"]
                }]
            }),
        );
        write_json(
            &form_root.join("calculations.json"),
            &json!({
                "calculations": [{
                    "calculation_id": "calc-a",
                    "source_refs": ["source#calc-a"]
                }]
            }),
        );
        write_json(
            &form_root.join("workflow.json"),
            &json!({
                "phases": [{
                    "phase": "edit",
                    "source_refs": ["source#phase-edit"]
                }],
                "transitions": [{
                    "action": "save",
                    "source_refs": ["source#transition-save"]
                }]
            }),
        );
        write_json(
            &form_root.join("fixtures/negative-cases.json"),
            &json!({
                "cases": [{
                    "case_id": "negative-a",
                    "source_refs": ["source#negative-a"]
                }]
            }),
        );
        fs::write(form_root.join("gaps.md"), "# No declared gaps\n").expect("write gaps");
        write_json(
            &repo.join("rules/index.json"),
            &json!({
                "forms": [{
                    "form_id": "form-a",
                    "form_code": "1234A",
                    "revision": "2026-01-01",
                    "package_version": "7.9.6.0",
                    "path": "forms/form-a/manifest.json"
                }]
            }),
        );

        let tracked = tracked_v1_sources(&form_root).expect("build tracked sources");
        let tracked_digest = digest_entries(
            TRACKED_V1_SOURCE_SET_DOMAIN,
            tracked
                .iter()
                .map(|source| (source.path.as_str(), source.canonical_bytes.as_slice())),
        );
        let field = inventory_record(1, Some("field-a"), "/fields/0", "source#field-a");
        let validation = inventory_record(1, Some("rule-a"), "/rules/0", "source#rule-a");
        let calculation = inventory_record(1, Some("calc-a"), "/calculations/0", "source#calc-a");
        let phase = inventory_record(1, Some("edit"), "/phases/0", "source#phase-edit");
        let transition =
            inventory_record(2, Some("save"), "/transitions/0", "source#transition-save");
        let fixture_record = inventory_record(
            1,
            Some("negative-a"),
            "negative-cases.json#/cases/0",
            "source#negative-a",
        );
        let summary = json!({
            "format": EVIDENCE_SUMMARY_FORMAT,
            "canonicalization": CANONICALIZATION_ID,
            "form_id": "form-a",
            "tracked_v1_source_set_sha256": tracked_digest,
            "tracked_sources": tracked.iter().map(|source| json!({
                "path": source.path.clone(),
                "size_bytes": source.canonical_bytes.len(),
                "sha256": sha256_hex(&source.canonical_bytes)
            })).collect::<Vec<_>>(),
            "upstream_assets": [],
            "capture_sessions": [],
            "source_excerpts": [],
            "capture_gaps": [],
            "dom_inventory": section(vec![field.clone()]),
            "xml_inventory": {
                "basis": "field-key-projection",
                "projected_field_key_count": 1,
                "declared_serializable_count": 2,
                "observed_occurrence_count": 0,
                "unresolved_occurrence_count": 2,
                "unresolved_count_delta": 1,
                "values_emitted": false,
                "records": [{
                    "ordinal": 1,
                    "key": "field-a",
                    "occurrence": null,
                    "observed": false,
                    "json_pointer": "/fields/0",
                    "source_refs": ["source#field-a"]
                }]
            },
            "runtime_observations": {},
            "save_finalize_reopen": {},
            "census": {
                "fields": section(vec![field.clone()]),
                "validations": section(vec![validation.clone()]),
                "calculations": section(vec![calculation.clone()]),
                "workflow": section(vec![phase, transition]),
                "serialization": section(vec![field]),
                "fixtures": section(vec![fixture_record]),
                "explicit_gaps": section(Vec::new()),
                "declared_counts": {
                    "fields": 1,
                    "validations": 1,
                    "calculations": 1,
                    "unverified_gaps": 0
                }
            }
        });
        let summary_bytes = canonical_json(&summary, "test summary").expect("serialize summary");
        let packet_root = root.join("packet");
        fs::create_dir(&packet_root).expect("create packet marker");
        let manifest = EvidencePacketManifest {
            format: EVIDENCE_PACKET_FORMAT.to_owned(),
            canonicalization: CANONICALIZATION_ID.to_owned(),
            packet_id: "packet-a".to_owned(),
            form_id: "form-a".to_owned(),
            rule_set_id: "form-a-p7.9.6.0".to_owned(),
            tracked_v1_source_set_sha256: tracked_digest,
            rule_set_source_state: RuleSetSourceState::Planned {
                source_set_sha256: (),
            },
            form_code: "1234A".to_owned(),
            form_revision: "2026-01-01".to_owned(),
            official_package_version: "7.9.6.0".to_owned(),
            official_package_evidence_id: "official-source".to_owned(),
            source_map_sha256: "3".repeat(64),
            source_verification_sha256: "4".repeat(64),
            capture_provenance: EvidenceCaptureProvenance {
                tool_commit: "a".repeat(40),
                command_argv: vec![
                    "bir-rules-codegen".to_owned(),
                    "verify-evidence-vault-source-map".to_owned(),
                    "--source-map".to_owned(),
                    "../evidence/source-map.json".to_owned(),
                ],
                capture_tool_version: "capture 1".to_owned(),
                operating_system: EvidenceCaptureOperatingSystem::Windows,
                windows_version: "Windows".to_owned(),
                official_app_version: "7.9.6.0".to_owned(),
                started_at_utc: "2026-07-26T00:00:00Z".to_owned(),
                finished_at_utc: "2026-07-26T00:01:00Z".to_owned(),
            },
            created_at_utc: "2026-07-26T00:00:00Z".to_owned(),
            review: EvidenceReview {
                status: EvidenceReviewStatus::Reviewed,
                reviewed_by: Some("reviewer".to_owned()),
                reviewed_at_utc: Some("2026-07-26T00:02:00Z".to_owned()),
            },
            attestations: Vec::new(),
            upstream_evidence: Vec::new(),
            derived_evidence: vec![DerivedEvidenceFile {
                path: SUMMARY_PATH.to_owned(),
                kind: DerivedEvidenceKind::RecordCensus,
                observation: EvidenceObservation::Observed,
                source_excerpt: None,
                media_type: "application/json".to_owned(),
                classification: "non-taxpayer-derived".to_owned(),
                review_status: EvidenceReviewStatus::Reviewed,
                source_evidence_ids: Vec::new(),
                size_bytes: summary_bytes.len() as u64,
                sha256: sha256_hex(&summary_bytes),
            }],
            packet_digest_sha256: "d".repeat(64),
        };
        let packet = VerifiedPacket {
            root: packet_root,
            manifest,
            derived_files: BTreeMap::from([(SUMMARY_PATH.to_owned(), summary_bytes)]),
            full_upstream_verified: false,
        };
        FactoryFixture {
            root,
            repo,
            form_root,
            packet,
        }
    }

    #[test]
    fn reviewed_planned_packet_emits_auditable_unresolved_skeleton_stably() {
        let fixture = fixture();
        let first = build_packet_backed_form_plan(
            &fixture.repo,
            &fixture.form_root,
            "form-a",
            &fixture.packet,
        )
        .expect("build reviewed skeleton");
        let second = build_packet_backed_form_plan(
            &fixture.repo,
            &fixture.form_root,
            "form-a",
            &fixture.packet,
        )
        .expect("rebuild reviewed skeleton");
        assert_eq!(first.files, second.files, "factory bytes must be stable");

        let rule_set: Value = serde_json::from_slice(
            first
                .files
                .get("rules/ir/v2/form-a-p7.9.6.0/rule-set.json")
                .expect("rule set"),
        )
        .expect("parse rule set");
        assert_eq!(rule_set["review_status"], "skeleton");
        assert!(rule_set["identity"]["source_set_sha256"].is_null());
        for key in [
            "fields",
            "rules",
            "calculations",
            "evaluation_order",
            "fixtures",
        ] {
            assert_eq!(rule_set[key], json!([]), "{key} remains non-executable");
        }
        assert_eq!(rule_set["serialization"]["artifacts"], json!([]));
        let classifications = rule_set["legacy_v1"]["record_classifications"]
            .as_array()
            .expect("classifications");
        assert_eq!(classifications.len(), 5);
        assert!(
            classifications
                .iter()
                .all(|entry| entry["outcome"] == "unresolved")
        );
        assert_eq!(
            classifications
                .iter()
                .filter(|entry| entry["artifact"] == "workflow")
                .count(),
            2
        );
        let handoff: Value =
            serde_json::from_slice(first.files.get("HANDOFF.json").expect("machine handoff"))
                .expect("parse handoff");
        assert_eq!(handoff["serialization_occurrences"]["blocking_gap"], true);
        assert_eq!(
            handoff["serialization_occurrences"]["unknown_occurrence_slots"]
                .as_array()
                .expect("unknown slots")
                .len(),
            1
        );

        let workspace = fixture.root.join("workspace");
        write_tree_atomically(&workspace, &first.files).expect("write external workspace");
        let report = audit(&AuditOptions::new(&workspace)).expect("audit skeleton workspace");
        assert_eq!(report.snapshot_count(), 1);
        assert_eq!(
            report
                .require_rule_set("form-a-p7.9.6.0")
                .expect("skeleton summary")
                .review_status(),
            "skeleton"
        );
    }

    #[test]
    fn candidate_pinned_identity_and_count_drift_fail_closed() {
        let mut fixture = fixture();
        fixture.packet.manifest.review.status = EvidenceReviewStatus::Candidate;
        let error = build_packet_backed_form_plan(
            &fixture.repo,
            &fixture.form_root,
            "form-a",
            &fixture.packet,
        )
        .expect_err("candidate packet must fail");
        assert!(error.to_string().contains("requires `reviewed`"));

        fixture.packet.manifest.review.status = EvidenceReviewStatus::Reviewed;
        fixture.packet.manifest.rule_set_source_state = RuleSetSourceState::Pinned {
            source_set_sha256: "e".repeat(64),
        };
        let error = build_packet_backed_form_plan(
            &fixture.repo,
            &fixture.form_root,
            "form-a",
            &fixture.packet,
        )
        .expect_err("pinned packet must fail");
        assert!(error.to_string().contains("planned/null"));

        fixture.packet.manifest.rule_set_source_state = RuleSetSourceState::Planned {
            source_set_sha256: (),
        };
        fixture.packet.manifest.form_code = "OTHER".to_owned();
        let error = build_packet_backed_form_plan(
            &fixture.repo,
            &fixture.form_root,
            "form-a",
            &fixture.packet,
        )
        .expect_err("identity drift must fail");
        assert!(error.to_string().contains("differs between rules index"));

        fixture.packet.manifest.form_code = "1234A".to_owned();
        let mut summary: Value = serde_json::from_slice(
            fixture
                .packet
                .derived_files
                .get(SUMMARY_PATH)
                .expect("summary"),
        )
        .expect("parse summary");
        summary["census"]["fields"]["count"] = json!(2);
        fixture.packet.derived_files.insert(
            SUMMARY_PATH.to_owned(),
            canonical_json(&summary, "drifted summary").expect("serialize drift"),
        );
        let error = build_packet_backed_form_plan(
            &fixture.repo,
            &fixture.form_root,
            "form-a",
            &fixture.packet,
        )
        .expect_err("count drift must fail");
        assert!(error.to_string().contains("exact ordered record bijection"));
    }

    #[test]
    fn canonical_destination_is_rejected_before_packet_materialization() {
        let fixture = fixture();
        let target = fixture.repo.join("rules/factory-output");
        let options = StageFormOptions::new(&fixture.repo, "form-a", &target)
            .with_packet(fixture.root.join("missing-packet"));
        let error = stage_form(&options).expect_err("canonical target must fail");
        assert!(error.to_string().contains("canonical rules"));
        assert!(!target.exists());
    }

    fn inventory_record(
        ordinal: usize,
        record_id: Option<&str>,
        json_pointer: &str,
        source_ref: &str,
    ) -> Value {
        json!({
            "ordinal": ordinal,
            "record_id": record_id,
            "json_pointer": json_pointer,
            "source_refs": [source_ref]
        })
    }

    fn section(records: Vec<Value>) -> Value {
        json!({"count": records.len(), "records": records})
    }

    fn write_json(path: &Path, value: &Value) {
        let parent = path.parent().expect("fixture path parent");
        fs::create_dir_all(parent).expect("create fixture parent");
        fs::write(
            path,
            canonical_json(value, "test fixture JSON").expect("serialize fixture"),
        )
        .expect("write fixture");
    }

    fn copy_schema_tree(repo: &Path) {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let target = repo.join("rules/schema/v2");
        for (relative, bytes) in read_tree(&source).expect("read v2 schemas") {
            let path = target.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
            fs::create_dir_all(path.parent().expect("schema parent"))
                .expect("create schema parent");
            fs::write(path, bytes).expect("copy schema");
        }
    }
}
