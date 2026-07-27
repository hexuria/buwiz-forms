//! Deterministic construction and drift checking for reviewed evidence packets.
//!
//! This module is deliberately separate from `evidence`: that module owns the
//! portable packet contract and verifier, while this module projects the
//! tracked v1 corpus into value-free packet payloads. Construction requires an
//! externally reviewed ledger and a content-addressed vault catalog. It never
//! reads the machine-local `official_assets.path` values in a v1 manifest.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use same_file::Handle;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::audit::discover_default_repo_root;
use crate::error::{CodegenError, Result};
use crate::evidence::{
    DerivedEvidenceFile, DerivedEvidenceKind, EVIDENCE_PACKET_DIGEST_DOMAIN,
    EVIDENCE_PACKET_FORMAT, EVIDENCE_PACKET_MANIFEST, EvidenceAttestation,
    EvidenceCaptureProvenance, EvidenceObservation, EvidencePacketManifest, EvidenceReview,
    EvidenceReviewStatus, RuleSetSourceState, SourceExcerptLocator, UpstreamEvidenceFile,
    VerifyEvidenceOptions, verify_evidence, verify_evidence_from_tree,
};
use crate::files::{
    ApprovedExternalFile, ApprovedExternalRoot, ReadScope, read_external_bytes_bound,
    read_external_bytes_under, read_external_tree_under, read_tracked_bytes, read_tracked_tree,
};
use crate::hash::{digest_entries, sha256_hex};
use crate::json::{CANONICALIZATION_ID, canonical_bytes, parse_strict};
use crate::path::{
    canonical_repo_root, ensure_under, is_same_or_below, is_same_path, is_symlink_or_reparse_point,
    portable_join, resolve_existing_under, validate_portable_relative,
};
use crate::sensitive::reject_sensitive_text;
use crate::vault_acquisition::{
    VaultAssetDisposition, validate_source_verifier_provenance, vault_asset_disposition,
};

pub const EVIDENCE_REVIEW_LEDGER_FORMAT: &str = "bir-evidence-review-ledger-v1";
pub const EVIDENCE_VAULT_CATALOG_FORMAT: &str = "bir-evidence-vault-catalog-v1";
pub const EVIDENCE_PACKET_SET_FORMAT: &str = "bir-evidence-packet-set-v1";
pub const EVIDENCE_PACKET_SET_MANIFEST: &str = "packet-set.json";
pub const EVIDENCE_SUMMARY_FORMAT: &str = "bir-evidence-derived-summary-v1";
pub const TRACKED_V1_SOURCE_SET_DOMAIN: &str = "bir-tracked-v1-source-set-v1";
pub const PACKET_SET_DIGEST_DOMAIN: &str = "bir-evidence-packet-set-digest-v1";
pub const PACKET_SET_ORDER_DOMAIN: &str = "bir-evidence-packet-set-order-v1";

const SUMMARY_PATH: &str = "derived/tracked-v1-summary.json";
const GAPS_PATH: &str = "derived/gaps.json";
const CONTENT_ADDRESS_PREFIX: &str = "upstream/sha256/";
const DERIVED_MEDIA_TYPE: &str = "application/json";
const DERIVED_CLASSIFICATION: &str = "non-taxpayer-derived";

#[derive(Clone, Debug)]
pub struct BuildEvidencePacketOptions {
    pub repo_root: PathBuf,
    pub form_id: String,
    pub review_ledger: PathBuf,
    pub vault_catalog: PathBuf,
    pub output_root: PathBuf,
    pub dry_run: bool,
    pub(crate) read_scope: ReadScope,
}

impl BuildEvidencePacketOptions {
    pub fn tracked_checkout(
        repo_root: impl Into<PathBuf>,
        form_id: impl Into<String>,
        review_ledger: impl Into<PathBuf>,
        vault_catalog: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            form_id: form_id.into(),
            review_ledger: review_ledger.into(),
            vault_catalog: vault_catalog.into(),
            output_root: output_root.into(),
            dry_run: false,
            read_scope: ReadScope::Tracked,
        }
    }

    pub fn external_workspace(
        repo_root: impl Into<PathBuf>,
        form_id: impl Into<String>,
        review_ledger: impl Into<PathBuf>,
        vault_catalog: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            form_id: form_id.into(),
            review_ledger: review_ledger.into(),
            vault_catalog: vault_catalog.into(),
            output_root: output_root.into(),
            dry_run: false,
            read_scope: ReadScope::External,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildEvidencePacketReport {
    pub form_id: String,
    pub packet_id: String,
    pub packet_digest_sha256: String,
    pub tracked_v1_source_set_sha256: String,
    pub derived_file_count: usize,
    pub upstream_file_count: usize,
    pub output_root: PathBuf,
    pub written: bool,
}

#[derive(Clone, Debug)]
pub struct StageEvidencePacketReviewOptions {
    pub repo_root: PathBuf,
    pub form_id: String,
    pub review_ledger: PathBuf,
    pub vault_catalog: PathBuf,
    pub output_root: PathBuf,
    pub(crate) read_scope: ReadScope,
}

impl StageEvidencePacketReviewOptions {
    pub fn tracked_checkout(
        repo_root: impl Into<PathBuf>,
        form_id: impl Into<String>,
        review_ledger: impl Into<PathBuf>,
        vault_catalog: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            form_id: form_id.into(),
            review_ledger: review_ledger.into(),
            vault_catalog: vault_catalog.into(),
            output_root: output_root.into(),
            read_scope: ReadScope::Tracked,
        }
    }

    pub fn external_workspace(
        repo_root: impl Into<PathBuf>,
        form_id: impl Into<String>,
        review_ledger: impl Into<PathBuf>,
        vault_catalog: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            form_id: form_id.into(),
            review_ledger: review_ledger.into(),
            vault_catalog: vault_catalog.into(),
            output_root: output_root.into(),
            read_scope: ReadScope::External,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BuildEvidencePacketSetOptions {
    pub repo_root: PathBuf,
    pub review_ledger: PathBuf,
    pub vault_catalog: PathBuf,
    pub output_root: PathBuf,
    pub dry_run: bool,
    pub(crate) read_scope: ReadScope,
}

impl BuildEvidencePacketSetOptions {
    pub fn tracked_checkout(
        repo_root: impl Into<PathBuf>,
        review_ledger: impl Into<PathBuf>,
        vault_catalog: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            review_ledger: review_ledger.into(),
            vault_catalog: vault_catalog.into(),
            output_root: output_root.into(),
            dry_run: false,
            read_scope: ReadScope::Tracked,
        }
    }

    pub fn external_workspace(
        repo_root: impl Into<PathBuf>,
        review_ledger: impl Into<PathBuf>,
        vault_catalog: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            review_ledger: review_ledger.into(),
            vault_catalog: vault_catalog.into(),
            output_root: output_root.into(),
            dry_run: false,
            read_scope: ReadScope::External,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildEvidencePacketSetReport {
    pub packet_count: usize,
    pub packet_set_digest_sha256: String,
    pub rules_index_sha256: String,
    pub packets: Vec<PlannedPacketDigest>,
    pub output_root: PathBuf,
    pub written: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PlannedPacketDigest {
    pub ordinal: usize,
    pub form_id: String,
    pub packet_id: String,
    pub packet_digest_sha256: String,
    pub manifest_sha256: String,
}

#[derive(Clone, Debug)]
pub struct CheckEvidencePacketSetOptions {
    pub repo_root: PathBuf,
    pub packet_root: PathBuf,
    pub vault_dir: Option<PathBuf>,
    pub(crate) read_scope: ReadScope,
}

impl CheckEvidencePacketSetOptions {
    pub fn tracked_checkout(
        repo_root: impl Into<PathBuf>,
        packet_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            packet_root: packet_root.into(),
            vault_dir: None,
            read_scope: ReadScope::Tracked,
        }
    }

    pub fn external_workspace(
        repo_root: impl Into<PathBuf>,
        packet_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            packet_root: packet_root.into(),
            vault_dir: None,
            read_scope: ReadScope::External,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckedPacket {
    pub ordinal: usize,
    pub form_id: String,
    pub packet_id: String,
    pub packet_digest_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckEvidencePacketSetReport {
    pub packet_count: usize,
    pub packet_set_digest_sha256: String,
    pub rules_index_sha256: String,
    pub full_upstream_verified: bool,
    pub packets: Vec<CheckedPacket>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewLedger {
    format: String,
    canonicalization: String,
    entries: Vec<ReviewLedgerEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewLedgerEntry {
    form_id: String,
    packet_id: String,
    rule_set_id: String,
    tracked_v1_source_set_sha256: String,
    rule_set_source_state: RuleSetSourceState,
    official_package_asset_id: String,
    capture_session_id: String,
    source_map_sha256: String,
    source_verification_sha256: String,
    capture_provenance: EvidenceCaptureProvenance,
    created_at_utc: String,
    review: EvidenceReview,
    attestations: Vec<EvidenceAttestation>,
    derived_reviews: Vec<DerivedReview>,
    source_excerpts: Vec<ReviewedSourceExcerpt>,
    capture_gaps: Vec<ReviewedCaptureGap>,
    expected_packet_digest_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DerivedReview {
    path: String,
    status: EvidenceReviewStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewedSourceExcerpt {
    excerpt_id: String,
    upstream_evidence_id: String,
    excerpt_start_byte: u64,
    excerpt_end_byte: u64,
    excerpt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewedCaptureGap {
    gap_id: String,
    reason: String,
    source_evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VaultCatalog {
    format: String,
    canonicalization: String,
    entries: Vec<VaultCatalogEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VaultCatalogEntry {
    evidence_id: String,
    sha256: String,
    size_bytes: u64,
    content_path: String,
    capture_session_id: String,
    source_map_sha256: String,
    source_verification_sha256: String,
    capture_provenance: EvidenceCaptureProvenance,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesIndex {
    #[serde(rename = "$schema")]
    schema: String,
    schema_version: String,
    knowledge_base: String,
    updated: String,
    forms: Vec<RulesIndexEntry>,
    priority_queue: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RulesIndexEntry {
    form_id: String,
    form_code: String,
    revision: String,
    package_version: String,
    priority: usize,
    status: String,
    path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct V2Index {
    #[serde(rename = "$schema")]
    schema: String,
    schema_version: String,
    snapshots: Vec<V2Snapshot>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct V2Snapshot {
    rule_set_id: String,
    form_code: String,
    form_revision: String,
    official_package_version: String,
    source_set_sha256: String,
    path: String,
    review_status: String,
    profile_states: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
struct SourceFile {
    path: String,
    canonical_bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct PacketPlan {
    manifest: EvidencePacketManifest,
    files: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LedgerMode {
    CandidateReview,
    ReviewedBuild,
}

#[derive(Clone, Debug)]
struct ManifestAsset {
    asset_id: String,
    kind: String,
    sha256: String,
    size_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
struct DerivedSummary {
    format: &'static str,
    canonicalization: &'static str,
    form_id: String,
    tracked_v1_source_set_sha256: String,
    tracked_sources: Vec<TrackedSourceSummary>,
    upstream_assets: Vec<UpstreamAssetSummary>,
    capture_sessions: Vec<CaptureSessionSummary>,
    source_excerpts: Vec<SourceExcerptSummary>,
    capture_gaps: Vec<GapSummary>,
    dom_inventory: InventorySection,
    xml_inventory: XmlInventory,
    runtime_observations: RuntimeObservationInventory,
    save_finalize_reopen: WorkflowInventory,
    census: RecordCensus,
}

#[derive(Clone, Debug, Serialize)]
struct TrackedSourceSummary {
    path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
struct UpstreamAssetSummary {
    asset_id: String,
    kind: String,
    disposition: VaultAssetDisposition,
    upstream_evidence_id: Option<String>,
    size_bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
struct CaptureSessionSummary {
    capture_session_id: String,
    source_map_sha256: String,
    source_verification_sha256: String,
    capture_provenance_sha256: String,
    upstream_evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct SourceExcerptSummary {
    excerpt_id: String,
    upstream_evidence_id: String,
    full_file_path: String,
    full_file_size_bytes: u64,
    full_file_sha256: String,
    excerpt_start_byte: u64,
    excerpt_end_byte: u64,
    excerpt_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
struct GapSummary {
    gap_id: String,
    reason: String,
    source_evidence_ids: Vec<String>,
    source_ref: String,
}

#[derive(Clone, Debug, Serialize)]
struct InventorySection {
    count: usize,
    records: Vec<InventoryRecord>,
}

#[derive(Clone, Debug, Serialize)]
struct InventoryRecord {
    ordinal: usize,
    record_id: Option<String>,
    json_pointer: String,
    source_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize)]
struct XmlRecord {
    ordinal: usize,
    key: String,
    occurrence: Option<usize>,
    observed: bool,
    json_pointer: String,
    source_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct RuntimeObservationInventory {
    observed_count: usize,
    observed: Vec<InventoryRecord>,
    source_derived_count: usize,
    source_derived_order: Vec<InventoryRecord>,
}

#[derive(Clone, Debug, Serialize)]
struct WorkflowInventory {
    state_count: usize,
    states: Vec<WorkflowStateRecord>,
    transition_count: usize,
    transitions: Vec<WorkflowTransitionRecord>,
    gap_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct WorkflowStateRecord {
    ordinal: usize,
    state_id: String,
    json_pointer: String,
    source_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct WorkflowTransitionRecord {
    ordinal: usize,
    from_state: Option<String>,
    action: Option<String>,
    to_state: Option<String>,
    json_pointer: String,
    source_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PacketSetManifest {
    format: String,
    canonicalization: String,
    rules_index_sha256: String,
    rules_index_order_sha256: String,
    packet_count: usize,
    packets: Vec<PacketSetEntry>,
    packet_set_digest_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PacketSetEntry {
    ordinal: usize,
    form_id: String,
    packet_id: String,
    path: String,
    packet_digest_sha256: String,
    manifest_sha256: String,
}

/// Builds one reviewed packet into a brand-new output directory.
pub fn build_evidence_packet(
    options: &BuildEvidencePacketOptions,
) -> Result<BuildEvidencePacketReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    reject_canonical_rules_target(&repo_root, &options.output_root)?;
    let context = BuildContext::load(
        &repo_root,
        &options.review_ledger,
        &options.vault_catalog,
        options.read_scope,
    )?;
    let index_entry = context
        .rules_index
        .forms
        .iter()
        .find(|entry| entry.form_id == options.form_id)
        .ok_or_else(|| {
            CodegenError::new(format!(
                "form_id `{}` is not present in rules/index.json",
                options.form_id
            ))
        })?;
    let ledger_entry = context.ledger_entry(&options.form_id)?;
    let plan = build_packet_plan(&context, index_entry, ledger_entry, !options.dry_run)?;
    let output_root = absolute_normalized(&options.output_root)?;
    if !options.dry_run {
        write_packet_fresh(&output_root, &plan)?;
    }

    Ok(BuildEvidencePacketReport {
        form_id: plan.manifest.form_id.clone(),
        packet_id: plan.manifest.packet_id.clone(),
        packet_digest_sha256: plan.manifest.packet_digest_sha256.clone(),
        tracked_v1_source_set_sha256: plan.manifest.tracked_v1_source_set_sha256.clone(),
        derived_file_count: plan.manifest.derived_evidence.len(),
        upstream_file_count: plan.manifest.upstream_evidence.len(),
        output_root,
        written: !options.dry_run,
    })
}

/// Writes a candidate packet to a fresh external directory for human review.
///
/// Candidate review metadata and candidate per-file statuses are mandatory;
/// the expected final reviewed digest must remain null. The result is a valid
/// portable candidate packet, so it can be inspected with `verify-evidence`,
/// but `import-evidence` and the aggregate checker reject it.
pub fn stage_evidence_packet_review(
    options: &StageEvidencePacketReviewOptions,
) -> Result<BuildEvidencePacketReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    reject_canonical_rules_target(&repo_root, &options.output_root)?;
    let context = BuildContext::load_with_mode(
        &repo_root,
        &options.review_ledger,
        &options.vault_catalog,
        LedgerMode::CandidateReview,
        options.read_scope,
    )?;
    let index_entry = context
        .rules_index
        .forms
        .iter()
        .find(|entry| entry.form_id == options.form_id)
        .ok_or_else(|| {
            CodegenError::new(format!(
                "form_id `{}` is not present in rules/index.json",
                options.form_id
            ))
        })?;
    let ledger_entry = context.ledger_entry(&options.form_id)?;
    let plan = build_packet_plan(&context, index_entry, ledger_entry, false)?;
    let output_root = absolute_normalized(&options.output_root)?;
    write_packet_fresh(&output_root, &plan)?;
    Ok(BuildEvidencePacketReport {
        form_id: plan.manifest.form_id.clone(),
        packet_id: plan.manifest.packet_id.clone(),
        packet_digest_sha256: plan.manifest.packet_digest_sha256.clone(),
        tracked_v1_source_set_sha256: plan.manifest.tracked_v1_source_set_sha256.clone(),
        derived_file_count: plan.manifest.derived_evidence.len(),
        upstream_file_count: plan.manifest.upstream_evidence.len(),
        output_root,
        written: true,
    })
}

/// Builds the exact rules/index.json packet set into a brand-new output root.
pub fn build_evidence_packet_set(
    options: &BuildEvidencePacketSetOptions,
) -> Result<BuildEvidencePacketSetReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    reject_canonical_rules_target(&repo_root, &options.output_root)?;
    let context = BuildContext::load(
        &repo_root,
        &options.review_ledger,
        &options.vault_catalog,
        options.read_scope,
    )?;
    context.require_exact_ledger_bijection()?;

    let mut files = BTreeMap::new();
    let mut set_entries = Vec::with_capacity(context.rules_index.forms.len());
    for (offset, index_entry) in context.rules_index.forms.iter().enumerate() {
        let ledger_entry = context.ledger_entry(&index_entry.form_id)?;
        let plan = build_packet_plan(&context, index_entry, ledger_entry, !options.dry_run)?;
        let packet_prefix = index_entry.form_id.clone();
        for (path, bytes) in &plan.files {
            files.insert(format!("{packet_prefix}/{path}"), bytes.clone());
        }
        let manifest_bytes = plan
            .files
            .get(EVIDENCE_PACKET_MANIFEST)
            .expect("packet plan always contains its manifest");
        set_entries.push(PacketSetEntry {
            ordinal: offset + 1,
            form_id: index_entry.form_id.clone(),
            packet_id: plan.manifest.packet_id.clone(),
            path: packet_prefix,
            packet_digest_sha256: plan.manifest.packet_digest_sha256.clone(),
            manifest_sha256: sha256_hex(manifest_bytes),
        });
    }

    let mut set_manifest = PacketSetManifest {
        format: EVIDENCE_PACKET_SET_FORMAT.to_owned(),
        canonicalization: CANONICALIZATION_ID.to_owned(),
        rules_index_sha256: context.rules_index_sha256.clone(),
        rules_index_order_sha256: rules_index_order_digest(&context.rules_index.forms)?,
        packet_count: set_entries.len(),
        packets: set_entries,
        packet_set_digest_sha256: String::new(),
    };
    set_manifest.packet_set_digest_sha256 = packet_set_digest(&set_manifest)?;
    files.insert(
        EVIDENCE_PACKET_SET_MANIFEST.to_owned(),
        canonical_serialize(&set_manifest, "packet set manifest")?,
    );

    let output_root = absolute_normalized(&options.output_root)?;
    if !options.dry_run {
        write_packet_set_fresh(&repo_root, &output_root, &files, options.read_scope)?;
    }
    let packets = set_manifest
        .packets
        .iter()
        .map(|packet| PlannedPacketDigest {
            ordinal: packet.ordinal,
            form_id: packet.form_id.clone(),
            packet_id: packet.packet_id.clone(),
            packet_digest_sha256: packet.packet_digest_sha256.clone(),
            manifest_sha256: packet.manifest_sha256.clone(),
        })
        .collect();
    Ok(BuildEvidencePacketSetReport {
        packet_count: set_manifest.packet_count,
        packet_set_digest_sha256: set_manifest.packet_set_digest_sha256,
        rules_index_sha256: set_manifest.rules_index_sha256,
        packets,
        output_root,
        written: !options.dry_run,
    })
}

/// Checks aggregate order/bijection/digests and each constituent packet.
pub fn check_evidence_packet_set(
    options: &CheckEvidencePacketSetOptions,
) -> Result<CheckEvidencePacketSetReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let packet_root = canonical_real_directory(&options.packet_root, "evidence packet root")?;
    check_packet_set_at(
        &repo_root,
        &packet_root,
        options.vault_dir.as_deref(),
        options.read_scope,
    )
}

struct BuildContext {
    repo_root: PathBuf,
    reader: ScopedReader,
    rules_index: RulesIndex,
    rules_index_sha256: String,
    v2_index: V2Index,
    ledger: ReviewLedger,
    catalog: VaultCatalog,
    catalog_by_tuple: BTreeMap<(String, u64), Vec<usize>>,
}

struct ScopedReader {
    scope: ReadScope,
    external_root: Option<ApprovedExternalRoot>,
}

impl ScopedReader {
    fn new(scope: ReadScope, root: &Path, label: &str) -> Result<Self> {
        let external_root = match scope {
            ReadScope::Tracked => None,
            ReadScope::External => Some(approve_exact_external_root(root, label, |_| Ok(()))?),
        };
        Ok(Self {
            scope,
            external_root,
        })
    }

    fn read_bytes(&self, path: &Path, label: &str) -> Result<Vec<u8>> {
        match self.scope {
            ReadScope::Tracked => read_tracked_bytes(path),
            ReadScope::External => {
                let root = self
                    .external_root
                    .as_ref()
                    .expect("external reader retains its approved root");
                read_external_bytes_under(root, path, label)
            }
        }
    }

    fn read_tree(&self, path: &Path, label: &str) -> Result<BTreeMap<String, Vec<u8>>> {
        match self.scope {
            ReadScope::Tracked => read_tracked_tree(path),
            ReadScope::External => {
                let root = self
                    .external_root
                    .as_ref()
                    .expect("external reader retains its approved root");
                root.revalidate(label)?;
                if !path.exists() {
                    root.revalidate(label)?;
                    return Ok(BTreeMap::new());
                }
                read_external_tree_under(root, path, label)
            }
        }
    }
}

impl BuildContext {
    fn load(
        repo_root: &Path,
        ledger_path: &Path,
        catalog_path: &Path,
        read_scope: ReadScope,
    ) -> Result<Self> {
        Self::load_with_mode(
            repo_root,
            ledger_path,
            catalog_path,
            LedgerMode::ReviewedBuild,
            read_scope,
        )
    }

    fn load_with_mode(
        repo_root: &Path,
        ledger_path: &Path,
        catalog_path: &Path,
        ledger_mode: LedgerMode,
        read_scope: ReadScope,
    ) -> Result<Self> {
        let reader = ScopedReader::new(read_scope, repo_root, "rules evidence workspace")?;
        let rules_index_path =
            resolve_existing_under(repo_root, "rules/index.json", "rules index")?;
        let rules_index_bytes = reader.read_bytes(&rules_index_path, "rules index")?;
        let rules_index_value = parse_strict(&rules_index_bytes, &rules_index_path)?;
        let rules_index_sha256 = sha256_hex(&canonical_bytes(&rules_index_value));
        let rules_index: RulesIndex = serde_json::from_value(rules_index_value.into_serde())
            .map_err(|source| {
                CodegenError::with_source("load closed rules/index.json structure", source)
            })?;
        validate_rules_index(&rules_index)?;

        let v2_index_path =
            resolve_existing_under(repo_root, "rules/ir/v2/index.json", "v2 rules index")?;
        let v2_index =
            load_scoped_typed_json::<V2Index>(&reader, &v2_index_path, false, "v2 rules index")?;
        validate_v2_index(&v2_index)?;

        let ledger = load_external_typed_json::<ReviewLedger>(
            ledger_path,
            true,
            "evidence packet review ledger",
        )?;
        validate_review_ledger(&ledger, ledger_mode)?;
        let catalog = load_external_typed_json::<VaultCatalog>(
            catalog_path,
            true,
            "evidence upstream vault catalog",
        )?;
        let catalog_by_tuple = validate_vault_catalog(&catalog)?;

        Ok(Self {
            repo_root: repo_root.to_path_buf(),
            reader,
            rules_index,
            rules_index_sha256,
            v2_index,
            ledger,
            catalog,
            catalog_by_tuple,
        })
    }

    fn ledger_entry(&self, form_id: &str) -> Result<&ReviewLedgerEntry> {
        self.ledger
            .entries
            .iter()
            .find(|entry| entry.form_id == form_id)
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "review ledger has no reviewed entry for form `{form_id}`"
                ))
            })
    }

    fn require_exact_ledger_bijection(&self) -> Result<()> {
        let expected: Vec<&str> = self
            .rules_index
            .forms
            .iter()
            .map(|entry| entry.form_id.as_str())
            .collect();
        let actual: Vec<&str> = self
            .ledger
            .entries
            .iter()
            .map(|entry| entry.form_id.as_str())
            .collect();
        if actual != expected {
            return Err(CodegenError::new(format!(
                "review ledger entries must be an exact ordered bijection with rules/index.json; expected=[{}] actual=[{}]",
                expected.join(", "),
                actual.join(", ")
            )));
        }
        Ok(())
    }
}

fn build_packet_plan(
    context: &BuildContext,
    index_entry: &RulesIndexEntry,
    ledger: &ReviewLedgerEntry,
    enforce_expected_digest: bool,
) -> Result<PacketPlan> {
    if ledger.form_id != index_entry.form_id {
        return Err(CodegenError::new(format!(
            "ledger form_id `{}` does not match index form_id `{}`",
            ledger.form_id, index_entry.form_id
        )));
    }
    let form_root = resolve_existing_under(
        &context.repo_root,
        &format!("rules/forms/{}", index_entry.form_id),
        "tracked v1 form root",
    )?;
    let sources = tracked_v1_sources(&form_root, &context.reader)?;
    let tracked_digest = digest_entries(
        TRACKED_V1_SOURCE_SET_DOMAIN,
        sources
            .iter()
            .map(|source| (source.path.as_str(), source.canonical_bytes.as_slice())),
    );
    if ledger.tracked_v1_source_set_sha256 != tracked_digest {
        return Err(CodegenError::new(format!(
            "stale/fake tracked v1 source digest for `{}`: ledger={} computed={tracked_digest}",
            index_entry.form_id, ledger.tracked_v1_source_set_sha256
        )));
    }

    validate_rule_set_identity(context, index_entry, ledger)?;
    let manifest_path = form_root.join("manifest.json");
    let manifest_value = load_scoped_json_value(&context.reader, &manifest_path)?;
    validate_manifest_identity(&manifest_value, index_entry)?;
    let assets = manifest_assets(&manifest_value)?;
    let selected = select_upstream_assets(context, &assets)?;
    validate_selected_capture_attribution(ledger, &selected)?;
    let official_package = assets
        .iter()
        .find(|asset| asset.asset_id == ledger.official_package_asset_id)
        .ok_or_else(|| {
            CodegenError::new(format!(
                "official_package_asset_id `{}` is not declared by `{}`",
                ledger.official_package_asset_id, index_entry.form_id
            ))
        })?;
    if official_package.size_bytes == 0 || official_package.kind != "official-package-executable" {
        return Err(CodegenError::new(format!(
            "official package asset `{}` must be a non-empty official-package-executable",
            official_package.asset_id
        )));
    }
    let official_package_catalog = selected
        .asset_to_catalog
        .get(&official_package.asset_id)
        .ok_or_else(|| {
            CodegenError::new(format!(
                "official package asset `{}` has no content-addressed vault entry",
                official_package.asset_id
            ))
        })?;

    let fields = load_required_form_document(&context.reader, &form_root, "fields.json")?;
    let validations = load_required_form_document(&context.reader, &form_root, "validations.json")?;
    let calculations =
        load_required_form_document(&context.reader, &form_root, "calculations.json")?;
    let workflow = load_required_form_document(&context.reader, &form_root, "workflow.json")?;
    let inventory = build_inventory(
        index_entry,
        &manifest_value,
        &fields,
        &validations,
        &calculations,
        &workflow,
        &form_root,
        &context.reader,
    )?;

    let upstream_files: Vec<UpstreamEvidenceFile> = selected
        .catalog_entries
        .iter()
        .map(|entry| UpstreamEvidenceFile {
            evidence_id: entry.evidence_id.clone(),
            path: entry.content_path.clone(),
            size_bytes: entry.size_bytes,
            sha256: entry.sha256.clone(),
        })
        .collect();
    let upstream_ids: Vec<String> = upstream_files
        .iter()
        .map(|entry| entry.evidence_id.clone())
        .collect();
    if upstream_ids.is_empty() {
        return Err(CodegenError::new(format!(
            "form `{}` has no non-empty content-addressed upstream evidence",
            index_entry.form_id
        )));
    }

    let excerpts = build_excerpt_summaries(ledger, &selected.catalog_entries)?;
    let mut gaps = build_gap_summaries(ledger, &assets, &upstream_ids)?;
    if inventory.xml_inventory.basis != "explicit-serialization-binding" {
        gaps.push(GapSummary {
            gap_id: "serialization-occurrences-not-observed".to_owned(),
            reason: format!(
                "No explicit value-free serialization binding inventory is tracked; {} field keys are projections, {} serializable occurrences are declared, and the occurrence-count delta is {}.",
                inventory.xml_inventory.projected_field_key_count,
                inventory
                    .xml_inventory
                    .declared_serializable_count
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_owned()),
                inventory.xml_inventory.unresolved_count_delta
            ),
            source_evidence_ids: upstream_ids.clone(),
            source_ref: "fields.json#/runtime_serializable_element_count".to_owned(),
        });
        gaps.sort_by(|left, right| left.gap_id.cmp(&right.gap_id));
    }
    for pair in gaps.windows(2) {
        if pair[0].gap_id == pair[1].gap_id {
            return Err(CodegenError::new(format!(
                "duplicate capture gap_id `{}` after deterministic gap projection",
                pair[0].gap_id
            )));
        }
    }
    if excerpts.is_empty() && gaps.is_empty() {
        return Err(CodegenError::new(format!(
            "review ledger for `{}` must provide a reviewed source excerpt or an explicit capture gap",
            index_entry.form_id
        )));
    }
    let capture_sessions = selected.capture_sessions;
    let ledger_provenance_sha256 = sha256_hex(&canonical_serialize(
        &ledger.capture_provenance,
        "review ledger capture provenance",
    )?);
    if let Some(existing) = capture_sessions
        .iter()
        .find(|session| session.capture_session_id == ledger.capture_session_id)
    {
        if existing.capture_provenance_sha256 != ledger_provenance_sha256 {
            return Err(CodegenError::new(format!(
                "capture_session_id `{}` binds conflicting capture provenance",
                ledger.capture_session_id
            )));
        }
    } else {
        return Err(CodegenError::new(format!(
            "review ledger capture_session_id `{}` is not bound by any selected vault catalog entry",
            ledger.capture_session_id
        )));
    }
    let summary = DerivedSummary {
        format: EVIDENCE_SUMMARY_FORMAT,
        canonicalization: CANONICALIZATION_ID,
        form_id: index_entry.form_id.clone(),
        tracked_v1_source_set_sha256: tracked_digest.clone(),
        tracked_sources: sources
            .iter()
            .map(|source| TrackedSourceSummary {
                path: source.path.clone(),
                size_bytes: source.canonical_bytes.len() as u64,
                sha256: sha256_hex(&source.canonical_bytes),
            })
            .collect(),
        upstream_assets: selected.asset_summaries,
        capture_sessions,
        source_excerpts: excerpts.clone(),
        capture_gaps: gaps.clone(),
        dom_inventory: inventory.dom_inventory,
        xml_inventory: inventory.xml_inventory,
        runtime_observations: inventory.runtime_observations,
        save_finalize_reopen: inventory.save_finalize_reopen,
        census: inventory.census,
    };
    let summary_bytes = canonical_serialize(&summary, "derived tracked v1 summary")?;
    let gap_bytes = canonical_serialize(
        &serde_json::json!({
            "capture_gaps": gaps,
            "format": "bir-evidence-gap-inventory-v1",
            "form_id": index_entry.form_id.as_str(),
        }),
        "derived gap inventory",
    )?;
    let mut derived_bytes = BTreeMap::from([
        (GAPS_PATH.to_owned(), gap_bytes),
        (SUMMARY_PATH.to_owned(), summary_bytes),
    ]);
    for excerpt in &excerpts {
        derived_bytes.insert(
            excerpt_path(&excerpt.excerpt_id),
            canonical_serialize(excerpt, "derived source excerpt locator")?,
        );
    }
    let review_by_path = validate_derived_reviews(
        &ledger.derived_reviews,
        derived_bytes.keys(),
        ledger.review.status,
    )?;

    let mut derived_evidence = Vec::with_capacity(derived_bytes.len());
    for (path, bytes) in &derived_bytes {
        let (kind, observation, source_excerpt, sources_for_file) = if path == SUMMARY_PATH {
            (
                DerivedEvidenceKind::RecordCensus,
                EvidenceObservation::Observed,
                None,
                upstream_ids.clone(),
            )
        } else if path == GAPS_PATH {
            let observation = if gaps.is_empty() {
                EvidenceObservation::Observed
            } else {
                EvidenceObservation::Gap {
                    reason: "The reviewed capture ledger records explicit evidence gaps; see the value-free gap inventory.".to_owned(),
                }
            };
            (
                DerivedEvidenceKind::GapReport,
                observation,
                None,
                upstream_ids.clone(),
            )
        } else {
            let excerpt = excerpts
                .iter()
                .find(|excerpt| excerpt_path(&excerpt.excerpt_id) == *path)
                .expect("every non-summary derived path was built from an excerpt");
            (
                DerivedEvidenceKind::SourceExcerpt,
                EvidenceObservation::Observed,
                Some(SourceExcerptLocator {
                    upstream_evidence_id: excerpt.upstream_evidence_id.clone(),
                    full_file_path: excerpt.full_file_path.clone(),
                    full_file_size_bytes: excerpt.full_file_size_bytes,
                    full_file_sha256: excerpt.full_file_sha256.clone(),
                    excerpt_start_byte: excerpt.excerpt_start_byte,
                    excerpt_end_byte: excerpt.excerpt_end_byte,
                    excerpt_sha256: excerpt.excerpt_sha256.clone(),
                }),
                vec![excerpt.upstream_evidence_id.clone()],
            )
        };
        derived_evidence.push(DerivedEvidenceFile {
            path: path.clone(),
            kind,
            observation,
            source_excerpt,
            media_type: DERIVED_MEDIA_TYPE.to_owned(),
            classification: DERIVED_CLASSIFICATION.to_owned(),
            review_status: *review_by_path
                .get(path)
                .expect("derived review bijection was checked"),
            source_evidence_ids: sources_for_file,
            size_bytes: bytes.len() as u64,
            sha256: sha256_hex(bytes),
        });
    }

    let mut packet_manifest = EvidencePacketManifest {
        format: EVIDENCE_PACKET_FORMAT.to_owned(),
        canonicalization: CANONICALIZATION_ID.to_owned(),
        packet_id: ledger.packet_id.clone(),
        form_id: index_entry.form_id.clone(),
        rule_set_id: ledger.rule_set_id.clone(),
        tracked_v1_source_set_sha256: tracked_digest,
        rule_set_source_state: ledger.rule_set_source_state.clone(),
        form_code: index_entry.form_code.clone(),
        form_revision: index_entry.revision.clone(),
        official_package_version: index_entry.package_version.clone(),
        official_package_evidence_id: official_package_catalog.evidence_id.clone(),
        source_map_sha256: ledger.source_map_sha256.clone(),
        source_verification_sha256: ledger.source_verification_sha256.clone(),
        capture_provenance: ledger.capture_provenance.clone(),
        created_at_utc: ledger.created_at_utc.clone(),
        review: ledger.review.clone(),
        attestations: ledger.attestations.clone(),
        upstream_evidence: upstream_files,
        derived_evidence,
        packet_digest_sha256: String::new(),
    };
    packet_manifest.packet_digest_sha256 =
        evidence_packet_digest(&packet_manifest, &derived_bytes)?;
    if enforce_expected_digest {
        let expected = ledger
            .expected_packet_digest_sha256
            .as_deref()
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "review ledger for `{}` has no expected packet digest; run the explicit dry-run, review its value-free plan, then bind that digest before building",
                    index_entry.form_id
                ))
            })?;
        if packet_manifest.packet_digest_sha256 != expected {
            return Err(CodegenError::new(format!(
                "stale review for `{}`: ledger packet digest={expected} computed={}",
                index_entry.form_id, packet_manifest.packet_digest_sha256
            )));
        }
    }
    let mut files = derived_bytes;
    files.insert(
        EVIDENCE_PACKET_MANIFEST.to_owned(),
        canonical_serialize(&packet_manifest, "evidence packet manifest")?,
    );
    Ok(PacketPlan {
        manifest: packet_manifest,
        files,
    })
}

struct SelectedAssets<'a> {
    catalog_entries: Vec<&'a VaultCatalogEntry>,
    asset_to_catalog: BTreeMap<String, &'a VaultCatalogEntry>,
    asset_summaries: Vec<UpstreamAssetSummary>,
    capture_sessions: Vec<CaptureSessionSummary>,
}

fn select_upstream_assets<'a>(
    context: &'a BuildContext,
    assets: &[ManifestAsset],
) -> Result<SelectedAssets<'a>> {
    let mut asset_to_catalog = BTreeMap::new();
    let mut selected_by_tuple: BTreeMap<(String, u64), &'a VaultCatalogEntry> = BTreeMap::new();
    let mut asset_summaries = Vec::new();
    for asset in assets {
        let disposition =
            vault_asset_disposition(&asset.kind, asset.size_bytes).map_err(|error| {
                CodegenError::new(format!(
                    "asset `{}` has no safe vault disposition: {error}",
                    asset.asset_id
                ))
            })?;
        let catalog_entry = match disposition {
            VaultAssetDisposition::Acquirable => {
                let key = (asset.sha256.clone(), asset.size_bytes);
                let indexes = context.catalog_by_tuple.get(&key).ok_or_else(|| {
                    CodegenError::new(format!(
                        "vault catalog has no content-addressed entry for asset `{}` ({}, {} bytes)",
                        asset.asset_id, asset.sha256, asset.size_bytes
                    ))
                })?;
                let catalog_entry = indexes
                    .iter()
                    .map(|index| &context.catalog.entries[*index])
                    .min_by_key(|entry| entry.evidence_id.as_str())
                    .expect("catalog tuple index is non-empty");
                selected_by_tuple.entry(key).or_insert(catalog_entry);
                asset_to_catalog.insert(asset.asset_id.clone(), catalog_entry);
                Some(catalog_entry)
            }
            VaultAssetDisposition::ZeroSizeProvenance
            | VaultAssetDisposition::MetadataOnlyTaxpayerPayload => None,
        };
        asset_summaries.push(UpstreamAssetSummary {
            asset_id: asset.asset_id.clone(),
            kind: asset.kind.clone(),
            disposition,
            upstream_evidence_id: catalog_entry.map(|entry| entry.evidence_id.clone()),
            size_bytes: asset.size_bytes,
            sha256: asset.sha256.clone(),
        });
    }
    asset_summaries.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    let mut catalog_entries: Vec<&VaultCatalogEntry> = selected_by_tuple.into_values().collect();
    catalog_entries.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));

    let mut sessions: BTreeMap<(String, String, String, String), BTreeSet<String>> =
        BTreeMap::new();
    for entry in &catalog_entries {
        let provenance_bytes =
            canonical_serialize(&entry.capture_provenance, "catalog capture provenance")?;
        sessions
            .entry((
                entry.capture_session_id.clone(),
                entry.source_map_sha256.clone(),
                entry.source_verification_sha256.clone(),
                sha256_hex(&provenance_bytes),
            ))
            .or_default()
            .insert(entry.evidence_id.clone());
    }
    let capture_sessions = sessions
        .into_iter()
        .map(
            |(
                (
                    capture_session_id,
                    source_map_sha256,
                    source_verification_sha256,
                    capture_provenance_sha256,
                ),
                evidence_ids,
            )| {
                CaptureSessionSummary {
                    capture_session_id,
                    source_map_sha256,
                    source_verification_sha256,
                    capture_provenance_sha256,
                    upstream_evidence_ids: evidence_ids.into_iter().collect(),
                }
            },
        )
        .collect();
    Ok(SelectedAssets {
        catalog_entries,
        asset_to_catalog,
        asset_summaries,
        capture_sessions,
    })
}

fn validate_selected_capture_attribution(
    ledger: &ReviewLedgerEntry,
    selected: &SelectedAssets<'_>,
) -> Result<()> {
    let expected_provenance_sha256 = sha256_hex(&canonical_serialize(
        &ledger.capture_provenance,
        "review ledger capture provenance",
    )?);
    for entry in &selected.catalog_entries {
        let observed_provenance_sha256 = sha256_hex(&canonical_serialize(
            &entry.capture_provenance,
            "catalog capture provenance",
        )?);
        if entry.capture_session_id != ledger.capture_session_id
            || entry.source_map_sha256 != ledger.source_map_sha256
            || entry.source_verification_sha256 != ledger.source_verification_sha256
            || observed_provenance_sha256 != expected_provenance_sha256
        {
            return Err(CodegenError::new(format!(
                "review ledger form `{}` capture attribution must exactly match every selected vault catalog entry",
                ledger.form_id
            )));
        }
    }
    Ok(())
}

struct BuiltInventory {
    dom_inventory: InventorySection,
    xml_inventory: XmlInventory,
    runtime_observations: RuntimeObservationInventory,
    save_finalize_reopen: WorkflowInventory,
    census: RecordCensus,
}

fn build_inventory(
    index: &RulesIndexEntry,
    manifest: &Value,
    fields: &Value,
    validations: &Value,
    calculations: &Value,
    workflow: &Value,
    form_root: &Path,
    reader: &ScopedReader,
) -> Result<BuiltInventory> {
    let field_records = records_from_array(fields, "fields", "field_key")?;
    let validation_records = records_from_array(validations, "rules", "rule_id")?;
    let calculation_records = records_from_array(calculations, "calculations", "calculation_id")?;
    verify_declared_count(
        manifest,
        "typed_fields",
        field_records.len(),
        &index.form_id,
    )?;
    verify_declared_count(
        manifest,
        "validation_rules",
        validation_records.len(),
        &index.form_id,
    )?;
    verify_declared_count(
        manifest,
        "calculations",
        calculation_records.len(),
        &index.form_id,
    )?;

    let dom_inventory = InventorySection {
        count: field_records.len(),
        records: field_records.clone(),
    };
    let (xml_inventory, serialization_records) =
        build_xml_inventory(form_root, fields, &field_records, reader)?;

    let rules = required_array(validations, "rules", "validations.json")?;
    let mut observed = Vec::new();
    let mut source_derived = Vec::new();
    for (index, value) in rules.iter().enumerate() {
        let record = inventory_record(value, index, "rules", "rule_id")?;
        if runtime_observed(value) {
            observed.push(record);
        } else {
            source_derived.push(record);
        }
    }
    let runtime_observations = RuntimeObservationInventory {
        observed_count: observed.len(),
        observed,
        source_derived_count: source_derived.len(),
        source_derived_order: source_derived,
    };

    let save_finalize_reopen = workflow_inventory(workflow)?;
    let workflow_records = generic_workflow_records(workflow)?;
    let fixture_records = fixture_inventory(form_root, reader)?;
    let declared_gap_count = manifest
        .pointer("/counts/unverified_gaps")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            CodegenError::new(format!(
                "{} manifest is missing counts.unverified_gaps",
                index.form_id
            ))
        })?;
    let explicit_gap_records = (0..declared_gap_count as usize)
        .map(|offset| InventoryRecord {
            ordinal: offset + 1,
            record_id: None,
            json_pointer: format!("/declared-gaps/{offset}"),
            source_refs: vec!["gaps.md".to_owned()],
        })
        .collect::<Vec<_>>();

    let mut declared_counts = BTreeMap::new();
    declared_counts.insert("fields".to_owned(), field_records.len() as u64);
    declared_counts.insert("validations".to_owned(), validation_records.len() as u64);
    declared_counts.insert("calculations".to_owned(), calculation_records.len() as u64);
    declared_counts.insert("unverified_gaps".to_owned(), declared_gap_count);
    let census = RecordCensus {
        fields: section(field_records),
        validations: section(validation_records),
        calculations: section(calculation_records),
        workflow: section(workflow_records),
        serialization: section(serialization_records),
        fixtures: section(fixture_records),
        explicit_gaps: section(explicit_gap_records),
        declared_counts,
    };
    Ok(BuiltInventory {
        dom_inventory,
        xml_inventory,
        runtime_observations,
        save_finalize_reopen,
        census,
    })
}

fn build_xml_inventory(
    form_root: &Path,
    fields_document: &Value,
    field_records: &[InventoryRecord],
    reader: &ScopedReader,
) -> Result<(XmlInventory, Vec<InventoryRecord>)> {
    let binding_path = form_root.join("fixtures/serialization-binding-inventory-v796.json");
    if binding_path.exists() {
        let document = load_scoped_json_value(reader, &binding_path)?;
        let bindings = required_array(
            &document,
            "occurrence_bindings",
            "serialization binding inventory",
        )?;
        let mut occurrences = BTreeMap::<String, usize>::new();
        let mut xml_records = Vec::with_capacity(bindings.len());
        let mut census_records = Vec::with_capacity(bindings.len());
        for (index, value) in bindings.iter().enumerate() {
            let object = value.as_object().ok_or_else(|| {
                CodegenError::new("serialization occurrence bindings must be objects")
            })?;
            let key =
                required_string(object, "key", "serialization occurrence binding")?.to_owned();
            let occurrence = occurrences.entry(key.clone()).or_insert(0);
            *occurrence += 1;
            let refs = source_refs(object)?;
            let pointer = format!(
                "fixtures/serialization-binding-inventory-v796.json#/occurrence_bindings/{index}"
            );
            xml_records.push(XmlRecord {
                ordinal: index + 1,
                key: key.clone(),
                occurrence: Some(*occurrence),
                observed: true,
                json_pointer: pointer.clone(),
                source_refs: refs.clone(),
            });
            census_records.push(InventoryRecord {
                ordinal: index + 1,
                record_id: Some(key),
                json_pointer: pointer,
                source_refs: refs,
            });
        }
        return Ok((
            XmlInventory {
                basis: "explicit-serialization-binding".to_owned(),
                projected_field_key_count: field_records.len(),
                declared_serializable_count: Some(bindings.len() as u64),
                observed_occurrence_count: bindings.len(),
                unresolved_occurrence_count: 0,
                unresolved_count_delta: 0,
                values_emitted: false,
                records: xml_records,
            },
            census_records,
        ));
    }

    let declared = fields_document
        .get("runtime_serializable_element_count")
        .and_then(Value::as_u64);
    let projected = field_records.len() as u64;
    let delta = declared
        .map(|declared| declared.abs_diff(projected))
        .unwrap_or(projected);
    let xml_records = field_records
        .iter()
        .map(|record| XmlRecord {
            ordinal: record.ordinal,
            key: record
                .record_id
                .clone()
                .expect("field records require field_key"),
            occurrence: None,
            observed: false,
            json_pointer: record.json_pointer.clone(),
            source_refs: record.source_refs.clone(),
        })
        .collect();
    Ok((
        XmlInventory {
            basis: "field-key-projection".to_owned(),
            projected_field_key_count: field_records.len(),
            declared_serializable_count: declared,
            observed_occurrence_count: 0,
            unresolved_occurrence_count: declared.unwrap_or(projected),
            unresolved_count_delta: delta,
            values_emitted: false,
            records: xml_records,
        },
        field_records.to_vec(),
    ))
}

fn section(records: Vec<InventoryRecord>) -> InventorySection {
    InventorySection {
        count: records.len(),
        records,
    }
}

fn records_from_array(document: &Value, key: &str, id_key: &str) -> Result<Vec<InventoryRecord>> {
    required_array(document, key, key)?
        .iter()
        .enumerate()
        .map(|(index, value)| inventory_record(value, index, key, id_key))
        .collect()
}

fn inventory_record(
    value: &Value,
    index: usize,
    array_key: &str,
    id_key: &str,
) -> Result<InventoryRecord> {
    let object = value.as_object().ok_or_else(|| {
        CodegenError::new(format!(
            "{array_key}[{index}] must be an object for evidence projection"
        ))
    })?;
    let record_id = object
        .get(id_key)
        .and_then(Value::as_str)
        .map(str::to_owned);
    if record_id.is_none() {
        return Err(CodegenError::new(format!(
            "{array_key}[{index}] is missing string `{id_key}`"
        )));
    }
    Ok(InventoryRecord {
        ordinal: index + 1,
        record_id,
        json_pointer: format!("/{array_key}/{index}"),
        source_refs: source_refs(object)?,
    })
}

fn source_refs(object: &Map<String, Value>) -> Result<Vec<String>> {
    let Some(value) = object.get("source_refs") else {
        return Ok(Vec::new());
    };
    let array = value
        .as_array()
        .ok_or_else(|| CodegenError::new("source_refs must be an array"))?;
    let mut refs = Vec::with_capacity(array.len());
    for value in array {
        let reference = value.as_str().ok_or_else(|| {
            CodegenError::new("v1 source_refs must contain strings for value-free projection")
        })?;
        reject_machine_locator(reference, "v1 source_ref")?;
        refs.push(reference.to_owned());
    }
    Ok(refs)
}

fn runtime_observed(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let evidence_type_observed = object
        .get("evidence_type")
        .and_then(Value::as_array)
        .is_some_and(|values| {
            values.iter().filter_map(Value::as_str).any(|value| {
                matches!(
                    value,
                    "ui-observation" | "runtime-observation" | "live-observation"
                )
            })
        });
    let source_observed = object
        .get("source_refs")
        .and_then(Value::as_array)
        .is_some_and(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .any(|value| value.to_ascii_lowercase().contains("observation"))
        });
    evidence_type_observed || source_observed
}

fn workflow_inventory(document: &Value) -> Result<WorkflowInventory> {
    let phases = required_array(document, "phases", "workflow.json")?;
    let mut states = Vec::with_capacity(phases.len());
    for (index, value) in phases.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| CodegenError::new("workflow phases must contain objects"))?;
        let state_id = object
            .get("phase")
            .and_then(Value::as_str)
            .ok_or_else(|| CodegenError::new("workflow phase is missing `phase`"))?;
        states.push(WorkflowStateRecord {
            ordinal: index + 1,
            state_id: state_id.to_owned(),
            json_pointer: format!("/phases/{index}"),
            source_refs: source_refs(object)?,
        });
    }
    let transitions = required_array(document, "transitions", "workflow.json")?;
    let mut transition_records = Vec::with_capacity(transitions.len());
    for (index, value) in transitions.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| CodegenError::new("workflow transitions must contain objects"))?;
        transition_records.push(WorkflowTransitionRecord {
            ordinal: index + 1,
            from_state: object
                .get("from")
                .and_then(Value::as_str)
                .map(str::to_owned),
            action: object
                .get("action")
                .and_then(Value::as_str)
                .map(str::to_owned),
            to_state: object.get("to").and_then(Value::as_str).map(str::to_owned),
            json_pointer: format!("/transitions/{index}"),
            source_refs: source_refs(object)?,
        });
    }
    let has_lifecycle_identifier = states
        .iter()
        .any(|record| contains_lifecycle_word(&record.state_id))
        || transition_records.iter().any(|record| {
            record
                .action
                .as_deref()
                .is_some_and(contains_lifecycle_word)
                || record
                    .from_state
                    .as_deref()
                    .is_some_and(contains_lifecycle_word)
                || record
                    .to_state
                    .as_deref()
                    .is_some_and(contains_lifecycle_word)
        });
    Ok(WorkflowInventory {
        state_count: states.len(),
        states,
        transition_count: transition_records.len(),
        transitions: transition_records,
        gap_ids: if has_lifecycle_identifier {
            Vec::new()
        } else {
            vec!["save-finalize-reopen-identifiers-not-tracked".to_owned()]
        },
    })
}

fn contains_lifecycle_word(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    ["save", "final", "reopen", "edit", "validate"]
        .iter()
        .any(|word| lower.contains(word))
}

fn generic_workflow_records(document: &Value) -> Result<Vec<InventoryRecord>> {
    let mut records = Vec::new();
    for (array_key, id_key) in [("phases", "phase"), ("transitions", "action")] {
        let array = required_array(document, array_key, "workflow.json")?;
        for (index, value) in array.iter().enumerate() {
            let object = value
                .as_object()
                .ok_or_else(|| CodegenError::new("workflow records must be objects"))?;
            records.push(InventoryRecord {
                ordinal: records.len() + 1,
                record_id: object
                    .get(id_key)
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                json_pointer: format!("/{array_key}/{index}"),
                source_refs: source_refs(object)?,
            });
        }
    }
    Ok(records)
}

fn fixture_inventory(form_root: &Path, reader: &ScopedReader) -> Result<Vec<InventoryRecord>> {
    let fixture_root = form_root.join("fixtures");
    let tree = reader.read_tree(&fixture_root, "tracked v1 fixture tree")?;
    let mut records = Vec::new();
    for (relative, bytes) in tree {
        if !relative.ends_with(".json") {
            continue;
        }
        let path = fixture_root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        let value = parse_strict(&bytes, &path)?.into_serde();
        let Some(object) = value.as_object() else {
            continue;
        };
        for (key, value) in object {
            let Some(array) = value.as_array() else {
                continue;
            };
            for (index, entry) in array.iter().enumerate() {
                let Some(entry_object) = entry.as_object() else {
                    continue;
                };
                let record_id = fixture_record_id(entry_object);
                records.push(InventoryRecord {
                    ordinal: records.len() + 1,
                    record_id,
                    json_pointer: format!("{relative}#/{key}/{index}"),
                    source_refs: source_refs(entry_object)?,
                });
            }
        }
    }
    Ok(records)
}

fn fixture_record_id(object: &Map<String, Value>) -> Option<String> {
    const ID_KEYS: [&str; 13] = [
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
    ];
    ID_KEYS
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str).map(str::to_owned))
}

fn build_excerpt_summaries(
    ledger: &ReviewLedgerEntry,
    upstream: &[&VaultCatalogEntry],
) -> Result<Vec<SourceExcerptSummary>> {
    let upstream_by_id: BTreeMap<&str, &&VaultCatalogEntry> = upstream
        .iter()
        .map(|entry| (entry.evidence_id.as_str(), entry))
        .collect();
    let mut excerpts = Vec::with_capacity(ledger.source_excerpts.len());
    let mut previous: Option<&str> = None;
    for excerpt in &ledger.source_excerpts {
        validate_portable_identifier(&excerpt.excerpt_id, "excerpt_id")?;
        if previous.is_some_and(|value| value >= excerpt.excerpt_id.as_str()) {
            return Err(CodegenError::new(
                "review ledger source_excerpts must be strictly ordered by excerpt_id",
            ));
        }
        previous = Some(&excerpt.excerpt_id);
        validate_sha256(&excerpt.excerpt_sha256, "excerpt_sha256")?;
        let entry = upstream_by_id
            .get(excerpt.upstream_evidence_id.as_str())
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "source excerpt `{}` cites upstream evidence `{}` not selected for this form",
                    excerpt.excerpt_id, excerpt.upstream_evidence_id
                ))
            })?;
        if excerpt.excerpt_start_byte >= excerpt.excerpt_end_byte
            || excerpt.excerpt_end_byte > entry.size_bytes
        {
            return Err(CodegenError::new(format!(
                "source excerpt `{}` has an invalid byte range",
                excerpt.excerpt_id
            )));
        }
        excerpts.push(SourceExcerptSummary {
            excerpt_id: excerpt.excerpt_id.clone(),
            upstream_evidence_id: entry.evidence_id.clone(),
            full_file_path: entry.content_path.clone(),
            full_file_size_bytes: entry.size_bytes,
            full_file_sha256: entry.sha256.clone(),
            excerpt_start_byte: excerpt.excerpt_start_byte,
            excerpt_end_byte: excerpt.excerpt_end_byte,
            excerpt_sha256: excerpt.excerpt_sha256.clone(),
        });
    }
    Ok(excerpts)
}

fn build_gap_summaries(
    ledger: &ReviewLedgerEntry,
    assets: &[ManifestAsset],
    upstream_ids: &[String],
) -> Result<Vec<GapSummary>> {
    let upstream_set: BTreeSet<&str> = upstream_ids.iter().map(String::as_str).collect();
    let mut gaps = Vec::new();
    let mut previous: Option<&str> = None;
    for gap in &ledger.capture_gaps {
        validate_portable_identifier(&gap.gap_id, "capture gap_id")?;
        if previous.is_some_and(|value| value >= gap.gap_id.as_str()) {
            return Err(CodegenError::new(
                "review ledger capture_gaps must be strictly ordered by gap_id",
            ));
        }
        previous = Some(&gap.gap_id);
        validate_safe_human_text(&gap.reason, "capture gap reason")?;
        let mut source_ids = gap.source_evidence_ids.clone();
        if source_ids.is_empty() {
            return Err(CodegenError::new(format!(
                "capture gap `{}` must cite at least one upstream evidence id",
                gap.gap_id
            )));
        }
        source_ids.sort();
        source_ids.dedup();
        if source_ids
            .iter()
            .any(|source| !upstream_set.contains(source.as_str()))
        {
            return Err(CodegenError::new(format!(
                "capture gap `{}` cites upstream evidence not selected for the form",
                gap.gap_id
            )));
        }
        gaps.push(GapSummary {
            gap_id: gap.gap_id.clone(),
            reason: gap.reason.clone(),
            source_evidence_ids: source_ids,
            source_ref: "review-ledger#/capture_gaps".to_owned(),
        });
    }
    for asset in assets {
        let disposition =
            vault_asset_disposition(&asset.kind, asset.size_bytes).map_err(|error| {
                CodegenError::new(format!(
                    "asset `{}` has no safe vault disposition while building gaps: {error}",
                    asset.asset_id
                ))
            })?;
        let (gap_prefix, explanation) = match disposition {
            VaultAssetDisposition::Acquirable => continue,
            VaultAssetDisposition::ZeroSizeProvenance => (
                "zero-size-provenance",
                "The tracked manifest records a zero-byte provenance identity; it is metadata only and is not a vault-held upstream file.",
            ),
            VaultAssetDisposition::MetadataOnlyTaxpayerPayload => (
                "metadata-only-taxpayer-payload",
                "The tracked manifest records a dummy save, final-copy, or taxpayer-shaped payload identity; safety policy forbids acquiring its bytes.",
            ),
        };
        let gap_id = format!("{gap_prefix}-{}", asset.asset_id);
        validate_portable_identifier(&gap_id, "metadata-only asset gap_id")?;
        gaps.push(GapSummary {
            gap_id,
            reason: format!(
                "{explanation} Declared kind={}, sha256={}, size_bytes={}.",
                asset.kind, asset.sha256, asset.size_bytes
            ),
            source_evidence_ids: Vec::new(),
            source_ref: format!("manifest.json#official_assets/{}", asset.asset_id),
        });
    }
    gaps.sort_by(|left, right| left.gap_id.cmp(&right.gap_id));
    for pair in gaps.windows(2) {
        if pair[0].gap_id == pair[1].gap_id {
            return Err(CodegenError::new(format!(
                "duplicate capture gap_id `{}`",
                pair[0].gap_id
            )));
        }
    }
    Ok(gaps)
}

fn validate_derived_reviews<'a, 'b>(
    reviews: &'a [DerivedReview],
    expected_paths: impl Iterator<Item = &'b String>,
    expected_status: EvidenceReviewStatus,
) -> Result<BTreeMap<String, EvidenceReviewStatus>> {
    let expected: Vec<&str> = expected_paths.map(String::as_str).collect();
    let actual: Vec<&str> = reviews.iter().map(|review| review.path.as_str()).collect();
    if actual != expected {
        return Err(CodegenError::new(format!(
            "derived_reviews must exactly match generated paths in order; expected=[{}] actual=[{}]",
            expected.join(", "),
            actual.join(", ")
        )));
    }
    let mut by_path = BTreeMap::new();
    for review in reviews {
        if review.status != expected_status
            || !matches!(
                review.status,
                EvidenceReviewStatus::Candidate | EvidenceReviewStatus::Reviewed
            )
        {
            return Err(CodegenError::new(format!(
                "derived review for `{}` must match packet review status `{expected_status:?}`",
                review.path,
            )));
        }
        by_path.insert(review.path.clone(), review.status);
    }
    Ok(by_path)
}

fn tracked_v1_sources(form_root: &Path, reader: &ScopedReader) -> Result<Vec<SourceFile>> {
    let tree = reader.read_tree(form_root, "tracked v1 form tree")?;
    let mut sources = Vec::new();
    for (path, bytes) in tree {
        let name = path.rsplit('/').next().unwrap_or(path.as_str());
        if matches!(name, "README.md" | "HANDOFF.md") || name.starts_with("v2-") {
            continue;
        }
        let canonical_bytes = if path.ends_with(".json") {
            let source_path = form_root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));
            canonical_bytes(&parse_strict(&bytes, &source_path)?)
        } else if path.ends_with(".md") {
            canonical_text(&bytes, &path)?
        } else {
            return Err(CodegenError::new(format!(
                "tracked v1 source `{path}` has unsupported non-text extension"
            )));
        };
        sources.push(SourceFile {
            path,
            canonical_bytes,
        });
    }
    let required = [
        "manifest.json",
        "fields.json",
        "validations.json",
        "calculations.json",
        "workflow.json",
        "gaps.md",
        "fixtures/negative-cases.json",
    ];
    let paths: BTreeSet<&str> = sources.iter().map(|source| source.path.as_str()).collect();
    for path in required {
        if !paths.contains(path) {
            return Err(CodegenError::new(format!(
                "tracked v1 source set is missing required `{path}`"
            )));
        }
    }
    Ok(sources)
}

fn canonical_text(bytes: &[u8], path: &str) -> Result<Vec<u8>> {
    let text = std::str::from_utf8(bytes).map_err(|source| {
        CodegenError::with_source(format!("tracked text source `{path}` is not UTF-8"), source)
    })?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    Ok(text.replace("\r\n", "\n").replace('\r', "\n").into_bytes())
}

fn validate_rule_set_identity(
    context: &BuildContext,
    index: &RulesIndexEntry,
    ledger: &ReviewLedgerEntry,
) -> Result<()> {
    validate_rule_set_source_for_identity(
        &context.v2_index,
        index,
        &ledger.rule_set_id,
        &ledger.rule_set_source_state,
    )
}

fn validate_rule_set_source_for_identity(
    v2_index: &V2Index,
    index: &RulesIndexEntry,
    rule_set_id: &str,
    source_state: &RuleSetSourceState,
) -> Result<()> {
    validate_rule_set_id(rule_set_id, &index.package_version)?;
    let snapshots: Vec<&V2Snapshot> = v2_index
        .snapshots
        .iter()
        .filter(|snapshot| {
            snapshot.form_code == index.form_code
                && snapshot.form_revision == index.revision
                && snapshot.official_package_version == index.package_version
        })
        .collect();
    match snapshots.as_slice() {
        [] => {
            if !matches!(
                source_state,
                RuleSetSourceState::Planned {
                    source_set_sha256: ()
                }
            ) {
                return Err(CodegenError::new(format!(
                    "pre-v2 form `{}` must use planned rule-set source state with null digest",
                    index.form_id
                )));
            }
        }
        [snapshot] => {
            if rule_set_id != snapshot.rule_set_id {
                return Err(CodegenError::new(format!(
                    "rule_set_id `{}` does not match tracked v2 snapshot `{}`",
                    rule_set_id, snapshot.rule_set_id
                )));
            }
            match source_state {
                RuleSetSourceState::Pinned { source_set_sha256 }
                    if source_set_sha256 == &snapshot.source_set_sha256 => {}
                _ => {
                    return Err(CodegenError::new(format!(
                        "tracked v2 form `{}` must use its real pinned source_set_sha256",
                        index.form_id
                    )));
                }
            }
        }
        _ => {
            return Err(CodegenError::new(format!(
                "multiple v2 snapshots match exact identity for `{}`",
                index.form_id
            )));
        }
    }
    Ok(())
}

fn validate_rule_set_id(value: &str, package_version: &str) -> Result<()> {
    validate_portable_identifier(value, "rule_set_id")?;
    let suffix = format!("-p{package_version}");
    if !value.ends_with(&suffix) {
        return Err(CodegenError::new(format!(
            "rule_set_id `{value}` must carry exact package suffix `{suffix}`"
        )));
    }
    Ok(())
}

fn validate_manifest_identity(manifest: &Value, index: &RulesIndexEntry) -> Result<()> {
    for (key, expected) in [
        ("form_id", index.form_id.as_str()),
        ("form_code", index.form_code.as_str()),
        ("revision", index.revision.as_str()),
        ("package_version", index.package_version.as_str()),
    ] {
        let actual = manifest
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| CodegenError::new(format!("form manifest is missing string `{key}`")))?;
        if actual != expected {
            return Err(CodegenError::new(format!(
                "form manifest `{key}` mismatch: index={expected} manifest={actual}"
            )));
        }
    }
    Ok(())
}

fn manifest_assets(manifest: &Value) -> Result<Vec<ManifestAsset>> {
    let assets = required_array(manifest, "official_assets", "form manifest")?;
    let mut result = Vec::with_capacity(assets.len());
    let mut ids = BTreeSet::new();
    for (index, value) in assets.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| CodegenError::new("official_assets entries must be objects"))?;
        // Deliberately do not access `path`: it is machine-local and non-operative.
        let asset_id = required_string(object, "asset_id", "official asset")?.to_owned();
        if !ids.insert(asset_id.clone()) {
            return Err(CodegenError::new(format!(
                "duplicate official asset_id `{asset_id}`"
            )));
        }
        let kind = required_string(object, "kind", "official asset")?.to_owned();
        let sha256 = required_string(object, "sha256", "official asset")?.to_owned();
        validate_sha256(&sha256, &format!("official_assets[{index}].sha256"))?;
        let size_bytes = object.get("size").and_then(Value::as_u64).ok_or_else(|| {
            CodegenError::new(format!("official_assets[{index}].size is not an integer"))
        })?;
        result.push(ManifestAsset {
            asset_id,
            kind,
            sha256,
            size_bytes,
        });
    }
    Ok(result)
}

fn validate_rules_index(index: &RulesIndex) -> Result<()> {
    let _metadata = (
        index.schema.as_str(),
        index.schema_version.as_str(),
        index.knowledge_base.as_str(),
        index.updated.as_str(),
    );
    if index.forms.len() != index.priority_queue.len() || index.forms.is_empty() {
        return Err(CodegenError::new(
            "rules/index.json forms and priority_queue must be non-empty and equal length",
        ));
    }
    let mut form_ids = BTreeSet::new();
    for (offset, entry) in index.forms.iter().enumerate() {
        if entry.priority != offset + 1 {
            return Err(CodegenError::new(
                "rules/index.json forms must already be in exact contiguous priority order",
            ));
        }
        if entry.form_code != index.priority_queue[offset] {
            return Err(CodegenError::new(format!(
                "rules/index.json priority_queue mismatch at ordinal {}",
                offset + 1
            )));
        }
        if !form_ids.insert(entry.form_id.as_str()) {
            return Err(CodegenError::new(format!(
                "duplicate rules/index form_id `{}`",
                entry.form_id
            )));
        }
    }
    Ok(())
}

fn validate_v2_index(index: &V2Index) -> Result<()> {
    let _metadata = (index.schema.as_str(), index.schema_version.as_str());
    let mut identities = BTreeSet::new();
    for snapshot in &index.snapshots {
        validate_sha256(&snapshot.source_set_sha256, "v2 source_set_sha256")?;
        if !identities.insert((
            snapshot.form_code.as_str(),
            snapshot.form_revision.as_str(),
            snapshot.official_package_version.as_str(),
        )) {
            return Err(CodegenError::new(
                "v2 index contains a duplicate exact form identity",
            ));
        }
        let _metadata = (
            snapshot.path.as_str(),
            snapshot.review_status.as_str(),
            &snapshot.profile_states,
        );
    }
    Ok(())
}

fn validate_review_ledger(ledger: &ReviewLedger, mode: LedgerMode) -> Result<()> {
    if ledger.format != EVIDENCE_REVIEW_LEDGER_FORMAT
        || ledger.canonicalization != CANONICALIZATION_ID
    {
        return Err(CodegenError::new(format!(
            "review ledger must use `{EVIDENCE_REVIEW_LEDGER_FORMAT}` and `{CANONICALIZATION_ID}`"
        )));
    }
    if ledger.entries.is_empty() {
        return Err(CodegenError::new("review ledger entries must not be empty"));
    }
    let mut ids = BTreeSet::new();
    let mut packet_ids = BTreeSet::new();
    for entry in &ledger.entries {
        validate_portable_identifier(&entry.form_id, "ledger form_id")?;
        validate_portable_identifier(&entry.packet_id, "ledger packet_id")?;
        validate_portable_identifier(
            &entry.official_package_asset_id,
            "official_package_asset_id",
        )?;
        validate_portable_identifier(&entry.capture_session_id, "capture_session_id")?;
        validate_sha256(&entry.source_map_sha256, "ledger source_map_sha256")?;
        validate_sha256(
            &entry.source_verification_sha256,
            "ledger source_verification_sha256",
        )?;
        validate_sha256(
            &entry.tracked_v1_source_set_sha256,
            "tracked_v1_source_set_sha256",
        )?;
        if let Some(expected) = &entry.expected_packet_digest_sha256 {
            validate_sha256(expected, "expected_packet_digest_sha256")?;
        }
        if !ids.insert(entry.form_id.as_str()) {
            return Err(CodegenError::new(format!(
                "review ledger has duplicate form_id `{}`",
                entry.form_id
            )));
        }
        if !packet_ids.insert(entry.packet_id.as_str()) {
            return Err(CodegenError::new(format!(
                "review ledger has duplicate packet_id `{}`",
                entry.packet_id
            )));
        }
        match mode {
            LedgerMode::CandidateReview => {
                if entry.review.status != EvidenceReviewStatus::Candidate
                    || entry.review.reviewed_by.is_some()
                    || entry.review.reviewed_at_utc.is_some()
                    || entry.expected_packet_digest_sha256.is_some()
                {
                    return Err(CodegenError::new(format!(
                        "candidate review entry `{}` requires candidate/null review metadata and a null expected packet digest",
                        entry.form_id
                    )));
                }
            }
            LedgerMode::ReviewedBuild => {
                if entry.review.status != EvidenceReviewStatus::Reviewed
                    || entry.review.reviewed_by.is_none()
                    || entry.review.reviewed_at_utc.is_none()
                {
                    return Err(CodegenError::new(format!(
                        "review ledger entry `{}` must be explicitly reviewed with reviewer and timestamp",
                        entry.form_id
                    )));
                }
                validate_safe_human_text(
                    entry
                        .review
                        .reviewed_by
                        .as_deref()
                        .expect("reviewed mode checked reviewer"),
                    "reviewed_by",
                )?;
                validate_utc_timestamp(
                    entry
                        .review
                        .reviewed_at_utc
                        .as_deref()
                        .expect("reviewed mode checked timestamp"),
                    "reviewed_at_utc",
                )?;
            }
        }
        validate_source_verifier_provenance(&entry.capture_provenance)?;
        validate_utc_timestamp(&entry.created_at_utc, "created_at_utc")?;
        validate_explicit_attestations(&entry.attestations)?;
        reject_machine_strings(
            &serde_json::to_value(entry).map_err(|source| {
                CodegenError::with_source("serialize review ledger entry for safety audit", source)
            })?,
            "review ledger",
        )?;
    }
    Ok(())
}

fn validate_vault_catalog(catalog: &VaultCatalog) -> Result<BTreeMap<(String, u64), Vec<usize>>> {
    if catalog.format != EVIDENCE_VAULT_CATALOG_FORMAT
        || catalog.canonicalization != CANONICALIZATION_ID
    {
        return Err(CodegenError::new(format!(
            "vault catalog must use `{EVIDENCE_VAULT_CATALOG_FORMAT}` and `{CANONICALIZATION_ID}`"
        )));
    }
    if catalog.entries.is_empty() {
        return Err(CodegenError::new("vault catalog entries must not be empty"));
    }
    let mut ids = BTreeSet::new();
    let mut tuples: BTreeMap<(String, u64), Vec<usize>> = BTreeMap::new();
    let mut catalog_capture_binding: Option<(String, String, String, String)> = None;
    let mut previous: Option<&str> = None;
    for (index, entry) in catalog.entries.iter().enumerate() {
        validate_portable_identifier(&entry.evidence_id, "catalog evidence_id")?;
        validate_portable_identifier(&entry.capture_session_id, "catalog capture_session_id")?;
        validate_sha256(&entry.sha256, "catalog sha256")?;
        validate_sha256(&entry.source_map_sha256, "catalog source_map_sha256")?;
        validate_sha256(
            &entry.source_verification_sha256,
            "catalog source_verification_sha256",
        )?;
        if entry.size_bytes == 0 {
            return Err(CodegenError::new(format!(
                "vault catalog entry `{}` must not claim a zero-byte upstream file",
                entry.evidence_id
            )));
        }
        if previous.is_some_and(|value| value >= entry.evidence_id.as_str()) {
            return Err(CodegenError::new(
                "vault catalog entries must be strictly ordered by evidence_id",
            ));
        }
        previous = Some(&entry.evidence_id);
        if !ids.insert(entry.evidence_id.as_str()) {
            return Err(CodegenError::new(format!(
                "duplicate vault catalog evidence_id `{}`",
                entry.evidence_id
            )));
        }
        let expected_id = format!("sha256-{}", entry.sha256);
        if entry.evidence_id != expected_id {
            return Err(CodegenError::new(format!(
                "vault catalog evidence_id `{}` must be exact content identity `{expected_id}`",
                entry.evidence_id
            )));
        }
        let expected_path = content_addressed_path(&entry.sha256);
        if entry.content_path != expected_path {
            return Err(CodegenError::new(format!(
                "vault catalog entry `{}` content_path must be `{expected_path}`",
                entry.evidence_id
            )));
        }
        reject_machine_strings(
            &serde_json::to_value(entry.capture_provenance.clone()).map_err(|source| {
                CodegenError::with_source("serialize catalog capture provenance", source)
            })?,
            "vault catalog capture provenance",
        )?;
        validate_source_verifier_provenance(&entry.capture_provenance)?;
        let provenance_sha256 = sha256_hex(&canonical_serialize(
            &entry.capture_provenance,
            "catalog capture provenance",
        )?);
        let capture_binding = (
            entry.capture_session_id.clone(),
            entry.source_map_sha256.clone(),
            entry.source_verification_sha256.clone(),
            provenance_sha256,
        );
        if catalog_capture_binding
            .as_ref()
            .is_some_and(|expected| expected != &capture_binding)
        {
            return Err(CodegenError::new(
                "vault catalog entries must share one capture session and exact source verification/provenance binding",
            ));
        }
        catalog_capture_binding = Some(capture_binding);
        tuples
            .entry((entry.sha256.clone(), entry.size_bytes))
            .or_default()
            .push(index);
    }
    Ok(tuples)
}

fn content_addressed_path(hash: &str) -> String {
    format!("{CONTENT_ADDRESS_PREFIX}{}/{}", &hash[..2], hash)
}

fn validate_derived_packet_manifest(
    manifest: &EvidencePacketManifest,
    index: &RulesIndexEntry,
    set_entry: &PacketSetEntry,
    v2_index: &V2Index,
    tracked_v1_source_set_sha256: &str,
) -> Result<()> {
    if manifest.review.status != EvidenceReviewStatus::Reviewed
        || manifest.review.reviewed_by.is_none()
        || manifest.review.reviewed_at_utc.is_none()
    {
        return Err(CodegenError::new(format!(
            "aggregate packet `{}` must be explicitly reviewed; candidate/rejected packets are review artifacts only",
            index.form_id
        )));
    }
    let non_reviewed: Vec<&str> = manifest
        .derived_evidence
        .iter()
        .filter(|file| file.review_status != EvidenceReviewStatus::Reviewed)
        .map(|file| file.path.as_str())
        .collect();
    if !non_reviewed.is_empty() {
        return Err(CodegenError::new(format!(
            "aggregate packet `{}` has non-reviewed derived files: {}",
            index.form_id,
            non_reviewed.join(", ")
        )));
    }
    if manifest.tracked_v1_source_set_sha256 != tracked_v1_source_set_sha256 {
        return Err(CodegenError::new(format!(
            "aggregate packet `{}` tracked v1 source drift: packet={} current={tracked_v1_source_set_sha256}",
            index.form_id, manifest.tracked_v1_source_set_sha256
        )));
    }
    validate_rule_set_source_for_identity(
        v2_index,
        index,
        &manifest.rule_set_id,
        &manifest.rule_set_source_state,
    )?;
    for (label, actual, expected) in [
        ("form_id", manifest.form_id.as_str(), index.form_id.as_str()),
        (
            "form_code",
            manifest.form_code.as_str(),
            index.form_code.as_str(),
        ),
        (
            "form_revision",
            manifest.form_revision.as_str(),
            index.revision.as_str(),
        ),
        (
            "official_package_version",
            manifest.official_package_version.as_str(),
            index.package_version.as_str(),
        ),
        (
            "packet_id",
            manifest.packet_id.as_str(),
            set_entry.packet_id.as_str(),
        ),
        (
            "packet_digest_sha256",
            manifest.packet_digest_sha256.as_str(),
            set_entry.packet_digest_sha256.as_str(),
        ),
    ] {
        if actual != expected {
            return Err(CodegenError::new(format!(
                "packet set `{}` mismatch for {}: expected `{expected}`, found `{actual}`",
                index.form_id, label
            )));
        }
    }
    Ok(())
}

fn check_packet_set_at(
    repo_root: &Path,
    packet_root: &Path,
    vault_dir: Option<&Path>,
    read_scope: ReadScope,
) -> Result<CheckEvidencePacketSetReport> {
    // Publication can start from a mapped-drive or other non-canonical
    // spelling even though child paths returned by `fs::canonicalize` use an
    // extended UNC spelling on Windows. Keep the containment root and every
    // resolved child in the same canonical namespace before comparing them.
    let packet_root = canonical_real_directory(packet_root, "evidence packet root")?;
    let reader = ScopedReader::new(read_scope, repo_root, "rules evidence workspace")?;
    let approved_packet_root =
        approve_exact_external_root(&packet_root, "evidence packet root", |_| Ok(()))?;
    let rules_index_path = resolve_existing_under(repo_root, "rules/index.json", "rules index")?;
    let rules_index_bytes = reader.read_bytes(&rules_index_path, "rules index")?;
    let rules_index_value = parse_strict(&rules_index_bytes, &rules_index_path)?;
    let rules_index_sha256 = sha256_hex(&canonical_bytes(&rules_index_value));
    let rules_index: RulesIndex =
        serde_json::from_value(rules_index_value.into_serde()).map_err(|source| {
            CodegenError::with_source("load closed rules/index.json structure", source)
        })?;
    validate_rules_index(&rules_index)?;
    let v2_index_path =
        resolve_existing_under(repo_root, "rules/ir/v2/index.json", "v2 rules index")?;
    let v2_index =
        load_scoped_typed_json::<V2Index>(&reader, &v2_index_path, false, "v2 rules index")?;
    validate_v2_index(&v2_index)?;

    let manifest_path = packet_root.join(EVIDENCE_PACKET_SET_MANIFEST);
    let set_manifest: PacketSetManifest = load_external_typed_json_under(
        &approved_packet_root,
        &manifest_path,
        true,
        "packet set manifest",
    )?;
    if set_manifest.format != EVIDENCE_PACKET_SET_FORMAT
        || set_manifest.canonicalization != CANONICALIZATION_ID
    {
        return Err(CodegenError::new(format!(
            "packet set must use `{EVIDENCE_PACKET_SET_FORMAT}` and `{CANONICALIZATION_ID}`"
        )));
    }
    if set_manifest.rules_index_sha256 != rules_index_sha256 {
        return Err(CodegenError::new(format!(
            "packet set rules index drift: manifest={} current={rules_index_sha256}",
            set_manifest.rules_index_sha256
        )));
    }
    let order_digest = rules_index_order_digest(&rules_index.forms)?;
    if set_manifest.rules_index_order_sha256 != order_digest {
        return Err(CodegenError::new(
            "packet set rules index ordered identity digest mismatch",
        ));
    }
    if set_manifest.packet_count != rules_index.forms.len()
        || set_manifest.packets.len() != rules_index.forms.len()
    {
        return Err(CodegenError::new(format!(
            "packet set must contain exactly {} packets",
            rules_index.forms.len()
        )));
    }
    let expected_top_level: BTreeSet<String> =
        std::iter::once(EVIDENCE_PACKET_SET_MANIFEST.to_owned())
            .chain(rules_index.forms.iter().map(|entry| entry.form_id.clone()))
            .collect();
    let actual_top_level = real_top_level_entries(&approved_packet_root)?;
    if actual_top_level != expected_top_level {
        let extra: Vec<&str> = actual_top_level
            .difference(&expected_top_level)
            .map(String::as_str)
            .collect();
        let missing: Vec<&str> = expected_top_level
            .difference(&actual_top_level)
            .map(String::as_str)
            .collect();
        return Err(CodegenError::new(format!(
            "packet root top-level bijection failed; extra=[{}] missing=[{}]",
            extra.join(", "),
            missing.join(", ")
        )));
    }

    let mut checked = Vec::with_capacity(rules_index.forms.len());
    let mut packet_ids = BTreeSet::new();
    let mut packet_paths = BTreeSet::new();
    for (offset, (index_entry, set_entry)) in rules_index
        .forms
        .iter()
        .zip(&set_manifest.packets)
        .enumerate()
    {
        if set_entry.ordinal != offset + 1
            || set_entry.form_id != index_entry.form_id
            || set_entry.path != index_entry.form_id
        {
            return Err(CodegenError::new(format!(
                "packet set order/bijection mismatch at ordinal {}",
                offset + 1
            )));
        }
        if !packet_ids.insert(set_entry.packet_id.as_str())
            || !packet_paths.insert(set_entry.path.as_str())
        {
            return Err(CodegenError::new(format!(
                "packet set contains a duplicate packet identity/path at ordinal {}",
                offset + 1
            )));
        }
        validate_portable_relative(&set_entry.path, "packet set packet path")?;
        validate_sha256(&set_entry.packet_digest_sha256, "packet digest")?;
        validate_sha256(&set_entry.manifest_sha256, "packet manifest sha256")?;
        let packet_dir =
            resolve_existing_under(&packet_root, &set_entry.path, "packet set packet directory")?;
        let packet_tree =
            read_external_tree_under(&approved_packet_root, &packet_dir, "evidence packet")?;
        let packet_manifest_path = packet_dir.join(EVIDENCE_PACKET_MANIFEST);
        let packet_manifest_bytes = packet_tree.get(EVIDENCE_PACKET_MANIFEST).ok_or_else(|| {
            CodegenError::new(format!(
                "evidence packet `{}` is missing `{EVIDENCE_PACKET_MANIFEST}`",
                packet_dir.display()
            ))
        })?;
        if sha256_hex(packet_manifest_bytes) != set_entry.manifest_sha256 {
            return Err(CodegenError::new(format!(
                "packet manifest drift for `{}`",
                index_entry.form_id
            )));
        }
        let packet_manifest: EvidencePacketManifest = parse_typed_json_bytes(
            packet_manifest_bytes,
            &packet_manifest_path,
            true,
            "evidence packet manifest",
        )?;
        let form_root = resolve_existing_under(
            repo_root,
            &format!("rules/forms/{}", index_entry.form_id),
            "tracked v1 form root",
        )?;
        let tracked_sources = tracked_v1_sources(&form_root, &reader)?;
        let tracked_digest = digest_entries(
            TRACKED_V1_SOURCE_SET_DOMAIN,
            tracked_sources
                .iter()
                .map(|source| (source.path.as_str(), source.canonical_bytes.as_slice())),
        );
        validate_derived_packet_manifest(
            &packet_manifest,
            index_entry,
            set_entry,
            &v2_index,
            &tracked_digest,
        )?;
        let mut verify_options = VerifyEvidenceOptions::new(&packet_dir);
        verify_options.vault_dir = vault_dir.map(Path::to_path_buf);
        let report = verify_evidence_from_tree(&verify_options, &packet_tree)?;
        if report.packet_digest_sha256 != set_entry.packet_digest_sha256 {
            return Err(CodegenError::new(format!(
                "verified packet digest drift for `{}`",
                index_entry.form_id
            )));
        }
        checked.push(CheckedPacket {
            ordinal: offset + 1,
            form_id: report.form_id,
            packet_id: report.packet_id,
            packet_digest_sha256: report.packet_digest_sha256,
        });
    }
    let computed_set_digest = packet_set_digest(&set_manifest)?;
    if computed_set_digest != set_manifest.packet_set_digest_sha256 {
        return Err(CodegenError::new(format!(
            "packet set aggregate digest mismatch: manifest={} computed={computed_set_digest}",
            set_manifest.packet_set_digest_sha256
        )));
    }
    approved_packet_root.revalidate("evidence packet root")?;
    Ok(CheckEvidencePacketSetReport {
        packet_count: checked.len(),
        packet_set_digest_sha256: set_manifest.packet_set_digest_sha256,
        rules_index_sha256,
        full_upstream_verified: vault_dir.is_some(),
        packets: checked,
    })
}

fn evidence_packet_digest(
    manifest: &EvidencePacketManifest,
    derived: &BTreeMap<String, Vec<u8>>,
) -> Result<String> {
    let mut normalized = manifest.clone();
    normalized.packet_digest_sha256.clear();
    let manifest_bytes = canonical_serialize(&normalized, "packet digest manifest")?;
    let mut entries = Vec::with_capacity(derived.len() + 1);
    entries.push((EVIDENCE_PACKET_MANIFEST.to_owned(), manifest_bytes));
    entries.extend(
        derived
            .iter()
            .map(|(path, bytes)| (path.clone(), bytes.clone())),
    );
    Ok(digest_entries(
        EVIDENCE_PACKET_DIGEST_DOMAIN,
        entries
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    ))
}

fn packet_set_digest(manifest: &PacketSetManifest) -> Result<String> {
    let mut normalized = manifest.clone();
    normalized.packet_set_digest_sha256.clear();
    let manifest_bytes = canonical_serialize(&normalized, "packet set digest manifest")?;
    let mut entries = Vec::with_capacity(manifest.packets.len() + 1);
    entries.push((EVIDENCE_PACKET_SET_MANIFEST.to_owned(), manifest_bytes));
    for packet in &manifest.packets {
        entries.push((
            format!("{}/packet-digest", packet.path),
            packet.packet_digest_sha256.as_bytes().to_vec(),
        ));
    }
    Ok(digest_entries(
        PACKET_SET_DIGEST_DOMAIN,
        entries
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    ))
}

fn rules_index_order_digest(entries: &[RulesIndexEntry]) -> Result<String> {
    let mut canonical_entries = Vec::with_capacity(entries.len());
    for entry in entries {
        canonical_entries.push(canonical_serialize(entry, "rules index identity")?);
    }
    let paths: Vec<String> = entries
        .iter()
        .enumerate()
        .map(|(offset, entry)| format!("{:04}-{}", offset + 1, entry.form_id))
        .collect();
    Ok(digest_entries(
        PACKET_SET_ORDER_DOMAIN,
        paths
            .iter()
            .zip(&canonical_entries)
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    ))
}

fn canonical_serialize(value: &impl Serialize, label: &str) -> Result<Vec<u8>> {
    let ordinary = serde_json::to_vec(value)
        .map_err(|source| CodegenError::with_source(format!("serialize {label}"), source))?;
    let parsed = parse_strict(&ordinary, Path::new(label))?;
    Ok(canonical_bytes(&parsed))
}

fn load_scoped_json_value(reader: &ScopedReader, path: &Path) -> Result<Value> {
    Ok(parse_strict(&reader.read_bytes(path, "scoped JSON document")?, path)?.into_serde())
}

fn load_scoped_typed_json<T>(
    reader: &ScopedReader,
    path: &Path,
    require_canonical: bool,
    label: &str,
) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let bytes = reader.read_bytes(path, label)?;
    parse_typed_json_bytes(&bytes, path, require_canonical, label)
}

fn load_external_typed_json<T>(path: &Path, require_canonical: bool, label: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let expected = fs::canonicalize(path)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label}"), path, source))?;
    let approved = ApprovedExternalFile::capture(path, label, |resolved| {
        if resolved != expected {
            return Err(CodegenError::new(format!(
                "{label} `{}` resolved to a different canonical file `{}`",
                expected.display(),
                resolved.display()
            )));
        }
        Ok(())
    })?;
    let bytes = read_external_bytes_bound(approved, label)?;
    parse_typed_json_bytes(&bytes, path, require_canonical, label)
}

fn load_external_typed_json_under<T>(
    root: &ApprovedExternalRoot,
    path: &Path,
    require_canonical: bool,
    label: &str,
) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let bytes = read_external_bytes_under(root, path, label)?;
    parse_typed_json_bytes(&bytes, path, require_canonical, label)
}

fn parse_typed_json_bytes<T>(
    bytes: &[u8],
    path: &Path,
    require_canonical: bool,
    label: &str,
) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let value = parse_strict(bytes, path)?;
    if require_canonical && bytes != canonical_bytes(&value) {
        return Err(CodegenError::new(format!(
            "{label} `{}` is not canonical `{CANONICALIZATION_ID}` JSON",
            path.display()
        )));
    }
    serde_json::from_value(value.into_serde()).map_err(|source| {
        CodegenError::with_source(
            format!(
                "closed-structure load of {label} `{}` failed",
                path.display()
            ),
            source,
        )
    })
}

fn load_required_form_document(
    reader: &ScopedReader,
    form_root: &Path,
    relative: &str,
) -> Result<Value> {
    let path = resolve_existing_under(form_root, relative, "tracked v1 form document")?;
    load_scoped_json_value(reader, &path)
}

fn required_array<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a Vec<Value>> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| CodegenError::new(format!("{label} is missing required array `{key}`")))
}

fn required_string<'a>(object: &'a Map<String, Value>, key: &str, label: &str) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| CodegenError::new(format!("{label} is missing required string `{key}`")))
}

fn verify_declared_count(manifest: &Value, key: &str, actual: usize, form_id: &str) -> Result<()> {
    let declared = manifest
        .pointer(&format!("/counts/{key}"))
        .and_then(Value::as_u64)
        .ok_or_else(|| CodegenError::new(format!("{form_id} manifest is missing counts.{key}")))?;
    if declared != actual as u64 {
        return Err(CodegenError::new(format!(
            "{form_id} manifest counts.{key}={declared} but projected records={actual}"
        )));
    }
    Ok(())
}

fn excerpt_path(excerpt_id: &str) -> String {
    format!("derived/source-excerpts/{excerpt_id}.json")
}

fn validate_portable_identifier(value: &str, label: &str) -> Result<()> {
    if value.is_empty() || value.len() > 128 {
        return Err(CodegenError::new(format!(
            "{label} must contain 1..=128 ASCII characters"
        )));
    }
    let mut bytes = value.bytes();
    let first = bytes.next().expect("empty identifiers were rejected");
    if !(first.is_ascii_lowercase() || first.is_ascii_digit())
        || !bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
        || value.contains("..")
        || value.ends_with('-')
        || value.ends_with('_')
        || value.ends_with('.')
    {
        return Err(CodegenError::new(format!(
            "{label} `{value}` is not a portable lowercase identifier"
        )));
    }
    Ok(())
}

fn validate_sha256(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CodegenError::new(format!(
            "{label} must be 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn validate_safe_human_text(value: &str, label: &str) -> Result<()> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > 1_024
        || value.chars().any(char::is_control)
    {
        return Err(CodegenError::new(format!(
            "{label} must be non-empty, trimmed, control-free text"
        )));
    }
    reject_machine_locator(value, label)?;
    reject_sensitive_text(value, label)
}

fn validate_explicit_attestations(attestations: &[EvidenceAttestation]) -> Result<()> {
    let expected = [
        crate::evidence::EvidenceAttestationKind::DerivedOnly,
        crate::evidence::EvidenceAttestationKind::NoTaxpayerValues,
        crate::evidence::EvidenceAttestationKind::NoCredentials,
        crate::evidence::EvidenceAttestationKind::NoOnlineSubmission,
    ];
    if attestations.len() != expected.len() {
        return Err(CodegenError::new(
            "review ledger must explicitly contain exactly four attestations",
        ));
    }
    for (attestation, expected_kind) in attestations.iter().zip(expected) {
        if attestation.kind != expected_kind || !attestation.attested {
            return Err(CodegenError::new(
                "review ledger attestations must be explicitly affirmed in contract order",
            ));
        }
        validate_safe_human_text(&attestation.attested_by, "attested_by")?;
        validate_utc_timestamp(&attestation.attested_at_utc, "attested_at_utc")?;
        validate_safe_human_text(&attestation.statement, "attestation statement")?;
    }
    Ok(())
}

fn validate_utc_timestamp(value: &str, label: &str) -> Result<()> {
    let bytes = value.as_bytes();
    let valid_shape = bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7 | 10 | 13 | 16 | 19) || byte.is_ascii_digit()
        });
    if !valid_shape {
        return Err(CodegenError::new(format!(
            "{label} must be an exact UTC timestamp `YYYY-MM-DDTHH:MM:SSZ`"
        )));
    }
    let component = |range: std::ops::Range<usize>| -> u32 {
        value[range]
            .parse()
            .expect("timestamp shape proved ASCII digits")
    };
    let year = component(0..4);
    let month = component(5..7);
    let day = component(8..10);
    let hour = component(11..13);
    let minute = component(14..16);
    let second = component(17..19);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    if year == 0 || day == 0 || day > days || hour > 23 || minute > 59 || second > 59 {
        return Err(CodegenError::new(format!(
            "{label} is not a real Gregorian UTC date and time"
        )));
    }
    Ok(())
}

fn reject_machine_strings(value: &Value, label: &str) -> Result<()> {
    match value {
        Value::Array(values) => {
            for value in values {
                reject_machine_strings(value, label)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                reject_machine_strings(value, label)?;
            }
        }
        Value::String(value) => {
            reject_machine_locator(value, label)?;
            reject_sensitive_text(value, label)?;
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn reject_machine_locator(value: &str, label: &str) -> Result<()> {
    let lower = value.to_ascii_lowercase();
    let bytes = value.as_bytes();
    let drive_path = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    if drive_path
        || value.starts_with("\\\\")
        || value.starts_with("//")
        || lower.starts_with("file:")
        || lower.starts_with("/users/")
        || lower.starts_with("/volumes/")
        || lower.contains("\\users\\")
    {
        return Err(CodegenError::new(format!(
            "{label} contains a machine-local path, which is non-operative evidence"
        )));
    }
    Ok(())
}

fn real_top_level_entries(root: &ApprovedExternalRoot) -> Result<BTreeSet<String>> {
    root.revalidate("evidence packet root")?;
    let entries = fs::read_dir(root.path())
        .map_err(|source| CodegenError::io("read packet root", root.path(), source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CodegenError::io("read packet root entry", root.path(), source))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| CodegenError::io("read packet root entry type", &path, source))?;
        if is_symlink_or_reparse_point(&metadata) || (!metadata.is_file() && !metadata.is_dir()) {
            return Err(CodegenError::new(format!(
                "packet root contains unsupported/symlink entry `{}`",
                path.display()
            )));
        }
        let name = entry.file_name().into_string().map_err(|_| {
            CodegenError::new(format!(
                "packet root entry `{}` is not valid UTF-8",
                path.display()
            ))
        })?;
        names.insert(name);
    }
    root.revalidate("evidence packet root")?;
    Ok(names)
}

fn write_packet_fresh(target: &Path, plan: &PacketPlan) -> Result<()> {
    write_fresh_fail_closed(target, &plan.files, |output| {
        let report = verify_evidence(&VerifyEvidenceOptions::new(output))?;
        if report.packet_digest_sha256 != plan.manifest.packet_digest_sha256 {
            return Err(CodegenError::new(
                "staged packet verification returned an unexpected digest",
            ));
        }
        Ok(())
    })
}

fn write_packet_set_fresh(
    repo_root: &Path,
    target: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    read_scope: ReadScope,
) -> Result<()> {
    write_fresh_fail_closed(target, files, |output| {
        check_packet_set_at(repo_root, output, None, read_scope)?;
        Ok(())
    })
}

fn write_fresh_fail_closed(
    target: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    verify: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    if target.exists() {
        return Err(CodegenError::new(format!(
            "output root `{}` already exists; refusing to overwrite",
            target.display()
        )));
    }
    reject_symlink_ancestors(target, "evidence output root")?;
    let parent = target.parent().ok_or_else(|| {
        CodegenError::new(format!("output root `{}` has no parent", target.display()))
    })?;
    fs::create_dir_all(parent)
        .map_err(|source| CodegenError::io("create evidence output parent", parent, source))?;
    let canonical_parent = canonical_real_directory(parent, "evidence output parent")?;
    let parent_identity = Handle::from_path(&canonical_parent).map_err(|source| {
        CodegenError::io("identify evidence output parent", &canonical_parent, source)
    })?;
    fs::create_dir(target)
        .map_err(|source| CodegenError::io("create fresh evidence output root", target, source))?;
    let output_identity = Handle::from_path(target)
        .map_err(|source| CodegenError::io("identify evidence output root", target, source))?;
    let canonical_output = fs::canonicalize(target)
        .map_err(|source| CodegenError::io("canonicalize evidence output root", target, source))?;
    if canonical_output
        .parent()
        .is_none_or(|observed| !is_same_path(observed, &canonical_parent))
    {
        return Err(CodegenError::new(
            "fresh evidence output was created outside its verified parent; it was left in place for manual inspection",
        ));
    }
    let operation = (|| {
        for (relative, bytes) in files {
            require_path_identity(target, &output_identity, "evidence output root")?;
            require_path_identity(
                &canonical_parent,
                &parent_identity,
                "evidence output parent",
            )?;
            let path = portable_join(target, relative, "evidence output path")?;
            ensure_under(target, &path, "evidence output file")?;
            let file_parent = path
                .parent()
                .expect("portable evidence output path has a parent");
            fs::create_dir_all(file_parent).map_err(|source| {
                CodegenError::io("create evidence output directory", file_parent, source)
            })?;
            reject_symlink_ancestors(&path, "evidence output file")?;
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(|source| CodegenError::io("create evidence output file", &path, source))?;
            file.write_all(bytes)
                .map_err(|source| CodegenError::io("write evidence output file", &path, source))?;
            file.sync_all()
                .map_err(|source| CodegenError::io("sync evidence output file", &path, source))?;
            let created_identity = Handle::from_file(file).map_err(|source| {
                CodegenError::io("identify created evidence output file", &path, source)
            })?;
            let current_identity = Handle::from_path(&path).map_err(|source| {
                CodegenError::io("reidentify evidence output file", &path, source)
            })?;
            if current_identity != created_identity {
                return Err(CodegenError::new(format!(
                    "evidence output file `{}` was replaced while it was being written",
                    path.display()
                )));
            }
            let canonical_path = fs::canonicalize(&path).map_err(|source| {
                CodegenError::io("canonicalize evidence output file", &path, source)
            })?;
            ensure_under(
                &canonical_output,
                &canonical_path,
                "canonical evidence output file",
            )?;
        }
        sync_directory(target)?;
        verify(target)?;
        require_path_identity(target, &output_identity, "evidence output root")?;
        require_path_identity(
            &canonical_parent,
            &parent_identity,
            "evidence output parent",
        )?;
        sync_directory(&canonical_parent)?;
        Ok(())
    })();
    operation.map_err(|error| {
        CodegenError::new(format!(
            "{error}; incomplete fresh evidence output `{}` was left in place to avoid deleting a concurrently substituted path",
            target.display()
        ))
    })
}

fn require_path_identity(path: &Path, expected: &Handle, label: &str) -> Result<()> {
    let current = Handle::from_path(path)
        .map_err(|source| CodegenError::io(&format!("reidentify {label}"), path, source))?;
    if &current != expected {
        return Err(CodegenError::new(format!(
            "{label} `{}` changed during publication",
            path.display()
        )));
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    match File::open(path).and_then(|directory| directory.sync_all()) {
        Ok(()) => Ok(()),
        Err(source) if cfg!(windows) => {
            let _ = source;
            Ok(())
        }
        Err(source) => Err(CodegenError::io(
            "sync evidence output directory",
            path,
            source,
        )),
    }
}

fn canonical_real_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| CodegenError::io(&format!("inspect {label}"), path, source))?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be a real directory",
            path.display()
        )));
    }
    fs::canonicalize(path)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label}"), path, source))
}

fn approve_exact_external_root(
    path: &Path,
    label: &str,
    validate_resolved_path: impl Fn(&Path) -> Result<()>,
) -> Result<ApprovedExternalRoot> {
    let expected = fs::canonicalize(path)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label}"), path, source))?;
    ApprovedExternalRoot::capture(path, label, |resolved| {
        if resolved != expected {
            return Err(CodegenError::new(format!(
                "{label} `{}` resolved to a different canonical root `{}`",
                expected.display(),
                resolved.display()
            )));
        }
        validate_resolved_path(resolved)
    })
}

fn absolute_normalized(path: &Path) -> Result<PathBuf> {
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(CodegenError::new(format!(
            "path `{}` must be lexically normalized",
            path.display()
        )));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        env::current_dir()
            .map(|directory| directory.join(path))
            .map_err(|source| CodegenError::with_source("read current directory", source))
    }
}

fn reject_canonical_rules_target(repo_root: &Path, target: &Path) -> Result<()> {
    let target = absolute_normalized(target)?;
    let rules = fs::canonicalize(repo_root.join("rules")).map_err(|source| {
        CodegenError::io(
            "canonicalize canonical rules directory",
            &repo_root.join("rules"),
            source,
        )
    })?;
    if is_same_or_below(&rules, &target) {
        return Err(CodegenError::new(format!(
            "evidence output `{}` must remain outside canonical rules `{}`",
            target.display(),
            rules.display()
        )));
    }
    let mut existing = target.as_path();
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| {
            CodegenError::new(format!(
                "evidence output `{}` has no existing ancestor",
                target.display()
            ))
        })?;
    }
    let resolved_ancestor = fs::canonicalize(existing).map_err(|source| {
        CodegenError::io("canonicalize evidence output ancestor", existing, source)
    })?;
    if is_same_or_below(&rules, &resolved_ancestor) {
        return Err(CodegenError::new(format!(
            "evidence output `{}` resolves beneath canonical rules `{}`",
            target.display(),
            rules.display()
        )));
    }
    Ok(())
}

fn reject_symlink_ancestors(path: &Path, label: &str) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if is_symlink_or_reparse_point(&metadata) => {
                return Err(CodegenError::new(format!(
                    "{label} `{}` traverses symlink `{}`",
                    path.display(),
                    ancestor.display()
                )));
            }
            Ok(_) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(CodegenError::io(
                    &format!("inspect {label} ancestor"),
                    ancestor,
                    source,
                ));
            }
        }
    }
    Ok(())
}

/// Command parser kept separate from the existing packet verifier parser.
pub fn run_evidence_set_command(
    command: &str,
    arguments: impl IntoIterator<Item = String>,
) -> Result<()> {
    let mut form_id = None;
    let mut review_ledger = None;
    let mut vault_catalog = None;
    let mut output_root = None;
    let mut packet_root = None;
    let mut vault = None;
    let mut repo_root = None;
    let mut json = false;
    let mut dry_run = false;
    let mut help = false;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--form-id"
                if matches!(
                    command,
                    "stage-evidence-packet-review" | "build-evidence-packet"
                ) =>
            {
                set_once(
                    &mut form_id,
                    next_cli_value(&mut arguments, "--form-id")?,
                    "--form-id",
                )?;
            }
            "--review-ledger"
                if matches!(
                    command,
                    "stage-evidence-packet-review"
                        | "build-evidence-packet"
                        | "build-evidence-packet-set"
                ) =>
            {
                set_once(
                    &mut review_ledger,
                    next_cli_value(&mut arguments, "--review-ledger")?,
                    "--review-ledger",
                )?;
            }
            "--vault-catalog"
                if matches!(
                    command,
                    "stage-evidence-packet-review"
                        | "build-evidence-packet"
                        | "build-evidence-packet-set"
                ) =>
            {
                set_once(
                    &mut vault_catalog,
                    next_cli_value(&mut arguments, "--vault-catalog")?,
                    "--vault-catalog",
                )?;
            }
            "--output-root"
                if matches!(
                    command,
                    "stage-evidence-packet-review"
                        | "build-evidence-packet"
                        | "build-evidence-packet-set"
                ) =>
            {
                set_once(
                    &mut output_root,
                    next_cli_value(&mut arguments, "--output-root")?,
                    "--output-root",
                )?;
            }
            "--packet-root" if command == "check-evidence-packet-set" => {
                set_once(
                    &mut packet_root,
                    next_cli_value(&mut arguments, "--packet-root")?,
                    "--packet-root",
                )?;
            }
            "--vault" if command == "check-evidence-packet-set" => {
                set_once(
                    &mut vault,
                    next_cli_value(&mut arguments, "--vault")?,
                    "--vault",
                )?;
            }
            "--json" if command == "check-evidence-packet-set" => json = true,
            "--dry-run"
                if matches!(
                    command,
                    "build-evidence-packet" | "build-evidence-packet-set"
                ) =>
            {
                dry_run = true;
            }
            "--repo-root" => {
                set_once(
                    &mut repo_root,
                    next_cli_value(&mut arguments, "--repo-root")?,
                    "--repo-root",
                )?;
            }
            "--help" | "-h" => help = true,
            other => {
                return Err(CodegenError::new(format!(
                    "unknown argument `{other}` for `{command}`\n\n{}",
                    evidence_set_usage(command)
                )));
            }
        }
    }
    if help {
        println!("{}", evidence_set_usage(command));
        return Ok(());
    }
    let repo_root = match repo_root {
        Some(path) => PathBuf::from(path),
        None => discover_default_repo_root()?,
    };
    match command {
        "stage-evidence-packet-review" => {
            let report =
                stage_evidence_packet_review(&StageEvidencePacketReviewOptions::tracked_checkout(
                    repo_root,
                    required_cli_value(form_id, "--form-id")?,
                    required_cli_path(review_ledger, "--review-ledger")?,
                    required_cli_path(vault_catalog, "--vault-catalog")?,
                    required_cli_path(output_root, "--output-root")?,
                ))?;
            println!(
                "staged candidate evidence packet {} for review at {}",
                report.packet_id,
                report.output_root.display()
            );
            println!("candidate packet digest: {}", report.packet_digest_sha256);
            println!("candidate packets cannot be imported or added to a checked packet set");
        }
        "build-evidence-packet" => {
            let mut options = BuildEvidencePacketOptions::tracked_checkout(
                repo_root,
                required_cli_value(form_id, "--form-id")?,
                required_cli_path(review_ledger, "--review-ledger")?,
                required_cli_path(vault_catalog, "--vault-catalog")?,
                required_cli_path(output_root, "--output-root")?,
            );
            options.dry_run = dry_run;
            let report = build_evidence_packet(&options)?;
            println!(
                "{} reviewed evidence packet {} for {} at {}",
                if report.written { "built" } else { "planned" },
                report.packet_id,
                report.form_id,
                report.output_root.display()
            );
            println!("packet digest: {}", report.packet_digest_sha256);
            println!(
                "tracked v1 source set: {}",
                report.tracked_v1_source_set_sha256
            );
        }
        "build-evidence-packet-set" => {
            if form_id.is_some() {
                return Err(CodegenError::new(
                    "`build-evidence-packet-set` does not accept --form-id",
                ));
            }
            let mut options = BuildEvidencePacketSetOptions::tracked_checkout(
                repo_root,
                required_cli_path(review_ledger, "--review-ledger")?,
                required_cli_path(vault_catalog, "--vault-catalog")?,
                required_cli_path(output_root, "--output-root")?,
            );
            options.dry_run = dry_run;
            let report = build_evidence_packet_set(&options)?;
            println!(
                "{} {}-packet evidence set at {}",
                if report.written { "built" } else { "planned" },
                report.packet_count,
                report.output_root.display()
            );
            for packet in &report.packets {
                println!(
                    "packet digest {}: {}",
                    packet.form_id, packet.packet_digest_sha256
                );
            }
            println!("packet set digest: {}", report.packet_set_digest_sha256);
        }
        "check-evidence-packet-set" => {
            let mut options = CheckEvidencePacketSetOptions::tracked_checkout(
                repo_root,
                required_cli_path(packet_root, "--packet-root")?,
            );
            options.vault_dir = vault.map(PathBuf::from);
            let report = check_evidence_packet_set(&options)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|source| {
                        CodegenError::with_source("serialize packet set check report", source)
                    })?
                );
            } else {
                println!(
                    "checked {} ordered evidence packet(s); set digest {}",
                    report.packet_count, report.packet_set_digest_sha256
                );
                println!("full upstream verified: {}", report.full_upstream_verified);
            }
        }
        _ => {
            return Err(CodegenError::new(format!(
                "unknown evidence set command `{command}`"
            )));
        }
    }
    Ok(())
}

pub fn evidence_set_usage(command: &str) -> String {
    match command {
        "stage-evidence-packet-review" => {
            "Usage: bir-rules-codegen stage-evidence-packet-review \
             --form-id ID --review-ledger FILE --vault-catalog FILE --output-root DIR \
             [--repo-root DIR]"
                .to_owned()
        }
        "build-evidence-packet" => "Usage: bir-rules-codegen build-evidence-packet \
             --form-id ID --review-ledger FILE --vault-catalog FILE --output-root DIR \
             [--dry-run] [--repo-root DIR]"
            .to_owned(),
        "build-evidence-packet-set" => "Usage: bir-rules-codegen build-evidence-packet-set \
             --review-ledger FILE --vault-catalog FILE --output-root DIR \
             [--dry-run] [--repo-root DIR]"
            .to_owned(),
        "check-evidence-packet-set" => "Usage: bir-rules-codegen check-evidence-packet-set \
             --packet-root DIR [--vault DIR] [--json] [--repo-root DIR]"
            .to_owned(),
        _ => "Evidence set commands: stage-evidence-packet-review, build-evidence-packet, build-evidence-packet-set, check-evidence-packet-set".to_owned(),
    }
}

fn set_once(slot: &mut Option<String>, value: String, flag: &str) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(CodegenError::new(format!(
            "{flag} may be provided only once"
        )));
    }
    Ok(())
}

fn next_cli_value(arguments: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    arguments
        .next()
        .ok_or_else(|| CodegenError::new(format!("{flag} requires a value")))
}

fn required_cli_path(value: Option<String>, flag: &str) -> Result<PathBuf> {
    value
        .map(PathBuf::from)
        .ok_or_else(|| CodegenError::new(format!("command requires {flag} PATH")))
}

fn required_cli_value(value: Option<String>, flag: &str) -> Result<String> {
    value.ok_or_else(|| CodegenError::new(format!("command requires {flag} VALUE")))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::{Value, json};

    use super::{
        BuildContext, BuildEvidencePacketOptions, BuildEvidencePacketSetOptions,
        CheckEvidencePacketSetOptions, DerivedReview, EVIDENCE_PACKET_SET_MANIFEST,
        EVIDENCE_REVIEW_LEDGER_FORMAT, EVIDENCE_VAULT_CATALOG_FORMAT, GAPS_PATH, ManifestAsset,
        PacketSetManifest, ReviewLedger, ReviewLedgerEntry, ReviewedCaptureGap, RuleSetSourceState,
        SUMMARY_PATH, ScopedReader, StageEvidencePacketReviewOptions, VaultAssetDisposition,
        VaultCatalog, VaultCatalogEntry, build_evidence_packet, build_evidence_packet_set,
        build_gap_summaries, build_packet_plan, canonical_serialize, check_evidence_packet_set,
        check_packet_set_at, content_addressed_path, evidence_packet_digest,
        load_external_typed_json, packet_set_digest, reject_canonical_rules_target,
        select_upstream_assets, sha256_hex, stage_evidence_packet_review, tracked_v1_sources,
        write_fresh_fail_closed,
    };
    use crate::evidence::{
        EvidenceAttestation, EvidenceAttestationKind, EvidenceCaptureOperatingSystem,
        EvidenceCaptureProvenance, EvidencePacketManifest, EvidenceReview, EvidenceReviewStatus,
        ImportEvidenceOptions, import_evidence,
    };
    use crate::files::{ReadScope, read_external_tree};
    use crate::hash::digest_entries;
    use crate::json::CANONICALIZATION_ID;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        ledger_path: PathBuf,
        catalog_path: PathBuf,
        ledger: ReviewLedger,
        upstream_bytes: Vec<u8>,
        upstream_hash: String,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            if self.root.exists() {
                fs::remove_dir_all(&self.root).expect("remove evidence-set fixture");
            }
        }
    }

    fn temporary_directory(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "bir-rules-codegen-evidence-set-{label}-{}-{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create test root");
        path
    }

    #[test]
    fn typed_external_json_rejects_hard_link_aliases_before_parsing() {
        let root = temporary_directory("typed-json-hard-link");
        let source = root.join("source.json");
        let alias = root.join("catalog.json");
        let bytes =
            canonical_serialize(&json!({"safe": true}), "test typed JSON").expect("serialize JSON");
        fs::write(&source, bytes).expect("write typed JSON");
        fs::hard_link(&source, &alias).expect("create hard-link alias");

        let error = load_external_typed_json::<Value>(&alias, true, "test catalog")
            .expect_err("hard-linked typed JSON must fail closed");
        assert!(error.to_string().contains("hard links"));

        fs::remove_dir_all(root).expect("remove typed JSON test root");
    }

    #[test]
    fn external_workspace_reader_rejects_or_blocks_root_substitution() {
        let root = temporary_directory("scoped-reader-root-substitution");
        let workspace = root.join("workspace");
        let displaced = root.join("workspace-displaced");
        fs::create_dir(&workspace).expect("create external workspace");
        let input = workspace.join("input.json");
        fs::write(&input, b"approved").expect("write approved workspace input");
        let reader = ScopedReader::new(ReadScope::External, &workspace, "test external workspace")
            .expect("approve external workspace");

        match fs::rename(&workspace, &displaced) {
            Ok(()) => {
                fs::create_dir(&workspace).expect("create replacement workspace");
                fs::write(workspace.join("input.json"), b"replacement")
                    .expect("write replacement workspace input");
                reader
                    .read_bytes(&workspace.join("input.json"), "test external input")
                    .expect_err("substituted workspace root must not authorize replacement bytes");
            }
            Err(_) => {
                assert_eq!(
                    reader
                        .read_bytes(&input, "test external input")
                        .expect("restrictive root handle blocks substitution"),
                    b"approved"
                );
            }
        }

        drop(reader);
        fs::remove_dir_all(root).expect("remove scoped reader fixture");
    }

    fn capture_provenance(_form_code: &str) -> EvidenceCaptureProvenance {
        EvidenceCaptureProvenance {
            tool_commit: "de828fd05ce27afa5c71ffd88c7a8bb2b3f9a8a5".to_owned(),
            command_argv: vec![
                "bir-rules-codegen".to_owned(),
                "verify-evidence-vault-source-map".to_owned(),
                "--source-map".to_owned(),
                "../evidence/source-map.json".to_owned(),
            ],
            capture_tool_version: "capture-evidence 1.0.0".to_owned(),
            operating_system: EvidenceCaptureOperatingSystem::Windows,
            windows_version: "Windows 11 24H2".to_owned(),
            official_app_version: "7.9.6.0".to_owned(),
            started_at_utc: "2026-07-26T00:00:00Z".to_owned(),
            finished_at_utc: "2026-07-26T00:01:00Z".to_owned(),
        }
    }

    fn attestations() -> Vec<EvidenceAttestation> {
        [
            EvidenceAttestationKind::DerivedOnly,
            EvidenceAttestationKind::NoTaxpayerValues,
            EvidenceAttestationKind::NoCredentials,
            EvidenceAttestationKind::NoOnlineSubmission,
        ]
        .into_iter()
        .map(|kind| EvidenceAttestation {
            kind,
            attested: true,
            attested_by: "reviewer-team".to_owned(),
            attested_at_utc: "2026-07-26T01:00:00Z".to_owned(),
            statement: format!("reviewed attestation {kind:?}"),
        })
        .collect()
    }

    fn write_json(path: &Path, value: &Value) {
        let parent = path.parent().expect("test JSON has parent");
        fs::create_dir_all(parent).expect("create test JSON parent");
        fs::write(
            path,
            canonical_serialize(value, "test JSON").expect("canonical test JSON"),
        )
        .expect("write test JSON");
    }

    fn write_typed_json(path: &Path, value: &impl serde::Serialize) {
        let parent = path.parent().expect("test JSON has parent");
        fs::create_dir_all(parent).expect("create test JSON parent");
        fs::write(
            path,
            canonical_serialize(value, "typed test JSON").expect("canonical typed test JSON"),
        )
        .expect("write typed test JSON");
    }

    fn fixture(form_count: usize, serializable_delta: u64) -> Fixture {
        let root = temporary_directory("repo");
        fs::create_dir_all(root.join("rules/ir/v2")).expect("create v2 index root");
        fs::create_dir_all(root.join("crates/bir-rules")).expect("create repository marker");
        write_json(
            &root.join("rules/ir/v2/index.json"),
            &json!({
                "$schema": "../../schema/v2/index.schema.json",
                "schema_version": "2.0.0",
                "snapshots": []
            }),
        );

        let upstream_bytes = b"reviewed official package bytes".to_vec();
        let upstream_hash = sha256_hex(&upstream_bytes);
        let catalog_entry = VaultCatalogEntry {
            evidence_id: format!("sha256-{upstream_hash}"),
            sha256: upstream_hash.clone(),
            size_bytes: upstream_bytes.len() as u64,
            content_path: content_addressed_path(&upstream_hash),
            capture_session_id: "catalog-session".to_owned(),
            source_map_sha256: "1".repeat(64),
            source_verification_sha256: "2".repeat(64),
            capture_provenance: capture_provenance("CATALOG"),
        };
        let catalog = VaultCatalog {
            format: EVIDENCE_VAULT_CATALOG_FORMAT.to_owned(),
            canonicalization: CANONICALIZATION_ID.to_owned(),
            entries: vec![catalog_entry],
        };
        let catalog_path = root.join("review/vault-catalog.json");
        write_typed_json(&catalog_path, &catalog);

        let mut index_forms = Vec::new();
        let mut priority_queue = Vec::new();
        let mut ledger_entries = Vec::new();
        let reader = ScopedReader::new(ReadScope::External, &root, "test rules workspace")
            .expect("approve test rules workspace");
        for ordinal in 1..=form_count {
            let form_id = format!("form-{ordinal:03}");
            let form_code = format!("F{ordinal:03}");
            let form_root = root.join(format!("rules/forms/{form_id}"));
            fs::create_dir_all(form_root.join("fixtures")).expect("create form fixture root");
            write_json(
                &form_root.join("manifest.json"),
                &json!({
                    "$schema": "../../schema/form-manifest.schema.json",
                    "schema_version": "1.0.0",
                    "form_id": form_id,
                    "form_code": form_code,
                    "revision": "2018-01-01",
                    "package_version": "7.9.6.0",
                    "status": "complete",
                    "official_assets": [{
                        "asset_id": "package-7.9.6",
                        "kind": "official-package-executable",
                        "path": "C:\\machine-local\\BIRForms.exe",
                        "sha256": upstream_hash,
                        "size": upstream_bytes.len(),
                        "revision_binding": "reviewed package"
                    }],
                    "counts": {
                        "typed_fields": 1,
                        "validation_rules": 1,
                        "calculations": 1,
                        "unverified_gaps": 0
                    },
                    "artifacts": {
                        "fields": "fields.json",
                        "validations": "validations.json",
                        "calculations": "calculations.json",
                        "workflow": "workflow.json",
                        "gaps": "gaps.md"
                    },
                    "scope_notes": []
                }),
            );
            write_json(
                &form_root.join("fields.json"),
                &json!({
                    "$schema": "../../schema/fields.schema.json",
                    "schema_version": "1.0.0",
                    "form_id": form_id,
                    "revision": "2018-01-01",
                    "field_count": 1,
                    "runtime_serializable_element_count": 1 + serializable_delta,
                    "fields": [{
                        "field_key": format!("{form_id}:field-a"),
                        "source_refs": ["package-7.9.6#field-a"]
                    }]
                }),
            );
            write_json(
                &form_root.join("validations.json"),
                &json!({
                    "$schema": "../../schema/validations.schema.json",
                    "schema_version": "1.0.0",
                    "form_id": form_id,
                    "revision": "2018-01-01",
                    "rules": [{
                        "rule_id": format!("{form_id}-rule-a"),
                        "order": 1,
                        "evidence_type": ["source"],
                        "source_refs": ["package-7.9.6#rule-a"]
                    }]
                }),
            );
            write_json(
                &form_root.join("calculations.json"),
                &json!({
                    "$schema": "../../schema/calculations.schema.json",
                    "schema_version": "1.0.0",
                    "form_id": form_id,
                    "revision": "2018-01-01",
                    "evaluation_order": [format!("{form_id}-calc-a")],
                    "calculations": [{
                        "calculation_id": format!("{form_id}-calc-a"),
                        "source_refs": ["package-7.9.6#calc-a"]
                    }]
                }),
            );
            write_json(
                &form_root.join("workflow.json"),
                &json!({
                    "$schema": "../../schema/workflow.schema.json",
                    "schema_version": "1.0.0",
                    "form_id": form_id,
                    "revision": "2018-01-01",
                    "phases": [{
                        "phase": "edit",
                        "source_refs": ["package-7.9.6#edit"]
                    }, {
                        "phase": "save",
                        "source_refs": ["package-7.9.6#save"]
                    }],
                    "transitions": [{
                        "from": "edit",
                        "action": "Save",
                        "to": "edit",
                        "source_refs": ["package-7.9.6#transition"]
                    }]
                }),
            );
            write_json(
                &form_root.join("fixtures/negative-cases.json"),
                &json!({
                    "$schema": "../../../schema/negative-fixtures.schema.json",
                    "schema_version": "1.0.0",
                    "form_id": form_id,
                    "cases": [{
                        "case_id": format!("{form_id}-case-a"),
                        "rule_id": format!("{form_id}-rule-a")
                    }]
                }),
            );
            fs::write(
                form_root.join("gaps.md"),
                "# Explicit gaps\n\nNone recorded.\r\n",
            )
            .expect("write gaps");
            fs::write(form_root.join("evidence.md"), "# Evidence\n").expect("write evidence");
            fs::write(form_root.join("audit.md"), "# Audit\n").expect("write audit");
            fs::write(form_root.join("README.md"), "# Non-source documentation\n")
                .expect("write excluded readme");

            let sources = tracked_v1_sources(&form_root, &reader).expect("tracked sources");
            let tracked_digest = digest_entries(
                super::TRACKED_V1_SOURCE_SET_DOMAIN,
                sources
                    .iter()
                    .map(|source| (source.path.as_str(), source.canonical_bytes.as_slice())),
            );
            index_forms.push(json!({
                "form_id": form_id,
                "form_code": form_code,
                "revision": "2018-01-01",
                "package_version": "7.9.6.0",
                "priority": ordinal,
                "status": "complete",
                "path": format!("forms/{form_id}/manifest.json")
            }));
            priority_queue.push(json!(form_code));
            ledger_entries.push(ReviewLedgerEntry {
                form_id: form_id.clone(),
                packet_id: format!("{form_id}-packet"),
                rule_set_id: format!("{form_id}-p7.9.6.0"),
                tracked_v1_source_set_sha256: tracked_digest,
                rule_set_source_state: RuleSetSourceState::Planned {
                    source_set_sha256: (),
                },
                official_package_asset_id: "package-7.9.6".to_owned(),
                capture_session_id: "catalog-session".to_owned(),
                source_map_sha256: "1".repeat(64),
                source_verification_sha256: "2".repeat(64),
                capture_provenance: capture_provenance("CATALOG"),
                created_at_utc: "2026-07-26T01:00:00Z".to_owned(),
                review: EvidenceReview {
                    status: EvidenceReviewStatus::Reviewed,
                    reviewed_by: Some("reviewer-team".to_owned()),
                    reviewed_at_utc: Some("2026-07-26T01:00:00Z".to_owned()),
                },
                attestations: attestations(),
                derived_reviews: vec![
                    DerivedReview {
                        path: GAPS_PATH.to_owned(),
                        status: EvidenceReviewStatus::Reviewed,
                    },
                    DerivedReview {
                        path: SUMMARY_PATH.to_owned(),
                        status: EvidenceReviewStatus::Reviewed,
                    },
                ],
                source_excerpts: Vec::new(),
                capture_gaps: vec![ReviewedCaptureGap {
                    gap_id: "source-excerpt-not-captured".to_owned(),
                    reason: "No source excerpt was reviewed for this value-free factory fixture."
                        .to_owned(),
                    source_evidence_ids: vec![format!("sha256-{upstream_hash}")],
                }],
                expected_packet_digest_sha256: None,
            });
        }
        write_json(
            &root.join("rules/index.json"),
            &json!({
                "$schema": "./schema/form-manifest.schema.json",
                "schema_version": "1.0.0",
                "knowledge_base": "test-validation-rules",
                "updated": "2026-07-26",
                "forms": index_forms,
                "priority_queue": priority_queue
            }),
        );
        let ledger = ReviewLedger {
            format: EVIDENCE_REVIEW_LEDGER_FORMAT.to_owned(),
            canonicalization: CANONICALIZATION_ID.to_owned(),
            entries: ledger_entries,
        };
        let ledger_path = root.join("review/review-ledger.json");
        write_typed_json(&ledger_path, &ledger);
        Fixture {
            root,
            ledger_path,
            catalog_path,
            ledger,
            upstream_bytes,
            upstream_hash,
        }
    }

    fn bind_expected_digests(fixture: &mut Fixture) {
        write_typed_json(&fixture.ledger_path, &fixture.ledger);
        let context = BuildContext::load(
            &fixture.root,
            &fixture.ledger_path,
            &fixture.catalog_path,
            ReadScope::External,
        )
        .expect("load build context");
        let digests: BTreeMap<String, String> = context
            .rules_index
            .forms
            .iter()
            .map(|index| {
                let ledger = context.ledger_entry(&index.form_id).expect("ledger entry");
                let plan =
                    build_packet_plan(&context, index, ledger, false).expect("plan packet digest");
                (index.form_id.clone(), plan.manifest.packet_digest_sha256)
            })
            .collect();
        for entry in &mut fixture.ledger.entries {
            entry.expected_packet_digest_sha256 = Some(
                digests
                    .get(&entry.form_id)
                    .expect("planned form digest")
                    .clone(),
            );
        }
        write_typed_json(&fixture.ledger_path, &fixture.ledger);
    }

    fn make_candidate(fixture: &mut Fixture) {
        for entry in &mut fixture.ledger.entries {
            entry.review = EvidenceReview {
                status: EvidenceReviewStatus::Candidate,
                reviewed_by: None,
                reviewed_at_utc: None,
            };
            entry.expected_packet_digest_sha256 = None;
            for review in &mut entry.derived_reviews {
                review.status = EvidenceReviewStatus::Candidate;
            }
        }
        write_typed_json(&fixture.ledger_path, &fixture.ledger);
    }

    fn write_vault(fixture: &Fixture, root: &Path) -> PathBuf {
        let vault = root.join("vault");
        let path = vault.join(
            content_addressed_path(&fixture.upstream_hash)
                .replace('/', std::path::MAIN_SEPARATOR_STR),
        );
        fs::create_dir_all(path.parent().expect("vault content parent"))
            .expect("create vault content root");
        fs::write(&path, &fixture.upstream_bytes).expect("write vault content");
        vault
    }

    #[test]
    fn dry_run_bootstraps_digest_without_writing_and_normal_build_is_stable() {
        let mut fixture = fixture(1, 0);
        let dry_target = fixture.root.join("dry-run-output");
        let mut dry_options = BuildEvidencePacketOptions::external_workspace(
            &fixture.root,
            "form-001",
            &fixture.ledger_path,
            &fixture.catalog_path,
            &dry_target,
        );
        dry_options.dry_run = true;
        let planned = build_evidence_packet(&dry_options).expect("plan reviewed packet");
        assert!(!planned.written);
        assert!(!dry_target.exists(), "dry-run must not create output");
        assert_eq!(
            fixture.ledger.entries[0].expected_packet_digest_sha256,
            None
        );

        fixture.ledger.entries[0].expected_packet_digest_sha256 =
            Some(planned.packet_digest_sha256.clone());
        write_typed_json(&fixture.ledger_path, &fixture.ledger);
        let first = fixture.root.join("packet-a");
        let second = fixture.root.join("packet-b");
        let report_a = build_evidence_packet(&BuildEvidencePacketOptions::external_workspace(
            &fixture.root,
            "form-001",
            &fixture.ledger_path,
            &fixture.catalog_path,
            &first,
        ))
        .expect("build first packet");
        let report_b = build_evidence_packet(&BuildEvidencePacketOptions::external_workspace(
            &fixture.root,
            "form-001",
            &fixture.ledger_path,
            &fixture.catalog_path,
            &second,
        ))
        .expect("build second packet");
        assert!(report_a.written && report_b.written);
        assert_eq!(report_a.packet_digest_sha256, planned.packet_digest_sha256);
        assert_eq!(
            read_external_tree(&first).expect("first packet tree"),
            read_external_tree(&second).expect("second packet tree")
        );
    }

    #[test]
    fn candidate_packet_is_inspectable_but_not_importable_or_buildable() {
        let mut fixture = fixture(1, 0);
        make_candidate(&mut fixture);
        let staged = fixture.root.join("candidate-review");
        let report =
            stage_evidence_packet_review(&StageEvidencePacketReviewOptions::external_workspace(
                &fixture.root,
                "form-001",
                &fixture.ledger_path,
                &fixture.catalog_path,
                &staged,
            ))
            .expect("stage candidate review packet");
        assert!(report.written);
        assert!(staged.join(super::EVIDENCE_PACKET_MANIFEST).is_file());

        let import_target = fixture.root.join("candidate-import");
        let error = import_evidence(&ImportEvidenceOptions::new(&staged, &import_target))
            .expect_err("candidate packet must not import");
        assert!(error.to_string().contains("requires `reviewed`"));
        assert!(!import_target.exists());

        let normal_target = fixture.root.join("reviewed-output");
        let error = build_evidence_packet(&BuildEvidencePacketOptions::external_workspace(
            &fixture.root,
            "form-001",
            &fixture.ledger_path,
            &fixture.catalog_path,
            &normal_target,
        ))
        .expect_err("normal builder must reject candidate ledger");
        assert!(error.to_string().contains("explicitly reviewed"));
        assert!(!normal_target.exists());
    }

    #[test]
    fn fake_source_digest_and_stale_packet_review_fail_before_output() {
        let mut fake_fixture = fixture(1, 0);
        fake_fixture.ledger.entries[0].tracked_v1_source_set_sha256 = "f".repeat(64);
        write_typed_json(&fake_fixture.ledger_path, &fake_fixture.ledger);
        let target = fake_fixture.root.join("must-not-exist");
        let mut options = BuildEvidencePacketOptions::external_workspace(
            &fake_fixture.root,
            "form-001",
            &fake_fixture.ledger_path,
            &fake_fixture.catalog_path,
            &target,
        );
        options.dry_run = true;
        let error = build_evidence_packet(&options).expect_err("fake source digest must fail");
        assert!(error.to_string().contains("stale/fake tracked v1"));
        assert!(!target.exists());

        let mut stale_fixture = fixture(1, 0);
        stale_fixture.ledger.entries[0].expected_packet_digest_sha256 = Some("f".repeat(64));
        write_typed_json(&stale_fixture.ledger_path, &stale_fixture.ledger);
        let target = stale_fixture.root.join("must-not-exist");
        let error = build_evidence_packet(&BuildEvidencePacketOptions::external_workspace(
            &stale_fixture.root,
            "form-001",
            &stale_fixture.ledger_path,
            &stale_fixture.catalog_path,
            &target,
        ))
        .expect_err("stale packet review must fail");
        assert!(error.to_string().contains("stale review"));
        assert!(!target.exists());

        let mut fixture = fixture(1, 0);
        fixture.ledger.entries[0].rule_set_source_state = RuleSetSourceState::Pinned {
            source_set_sha256: "f".repeat(64),
        };
        write_typed_json(&fixture.ledger_path, &fixture.ledger);
        let mut options = BuildEvidencePacketOptions::external_workspace(
            &fixture.root,
            "form-001",
            &fixture.ledger_path,
            &fixture.catalog_path,
            fixture.root.join("must-not-exist"),
        );
        options.dry_run = true;
        let error = build_evidence_packet(&options)
            .expect_err("pre-v2 form must not invent a pinned source digest");
        assert!(error.to_string().contains("planned rule-set source state"));
    }

    #[test]
    fn vault_catalog_must_match_content_and_cannot_carry_machine_paths() {
        let machine_path_fixture = fixture(1, 0);
        let mut catalog: VaultCatalog = serde_json::from_slice(
            &fs::read(&machine_path_fixture.catalog_path).expect("read catalog"),
        )
        .expect("parse catalog");
        catalog.entries[0]
            .capture_provenance
            .command_argv
            .push("C:\\machine-local\\capture-output.json".to_owned());
        write_typed_json(&machine_path_fixture.catalog_path, &catalog);
        let mut options = BuildEvidencePacketOptions::external_workspace(
            &machine_path_fixture.root,
            "form-001",
            &machine_path_fixture.ledger_path,
            &machine_path_fixture.catalog_path,
            machine_path_fixture.root.join("dry"),
        );
        options.dry_run = true;
        let error = build_evidence_packet(&options).expect_err("machine path must fail");
        assert!(error.to_string().contains("machine-local path"));

        let missing_tuple_fixture = fixture(1, 0);
        let mut catalog: VaultCatalog = serde_json::from_slice(
            &fs::read(&missing_tuple_fixture.catalog_path).expect("read catalog"),
        )
        .expect("parse catalog");
        catalog.entries[0].sha256 = "e".repeat(64);
        catalog.entries[0].evidence_id = format!("sha256-{}", catalog.entries[0].sha256);
        catalog.entries[0].content_path = content_addressed_path(&catalog.entries[0].sha256);
        write_typed_json(&missing_tuple_fixture.catalog_path, &catalog);
        let mut options = BuildEvidencePacketOptions::external_workspace(
            &missing_tuple_fixture.root,
            "form-001",
            &missing_tuple_fixture.ledger_path,
            &missing_tuple_fixture.catalog_path,
            missing_tuple_fixture.root.join("dry"),
        );
        options.dry_run = true;
        let error = build_evidence_packet(&options).expect_err("missing content tuple must fail");
        assert!(error.to_string().contains("no content-addressed entry"));
    }

    #[test]
    fn upstream_assets_are_deduplicated_by_hash_and_size() {
        let fixture = fixture(1, 0);
        let context = BuildContext::load(
            &fixture.root,
            &fixture.ledger_path,
            &fixture.catalog_path,
            ReadScope::External,
        )
        .expect("load build context");
        let assets = vec![
            ManifestAsset {
                asset_id: "installer-primary".to_owned(),
                kind: "official-package-executable".to_owned(),
                sha256: fixture.upstream_hash.clone(),
                size_bytes: fixture.upstream_bytes.len() as u64,
            },
            ManifestAsset {
                asset_id: "installer-alias".to_owned(),
                kind: "official-package-executable".to_owned(),
                sha256: fixture.upstream_hash.clone(),
                size_bytes: fixture.upstream_bytes.len() as u64,
            },
        ];

        let selected = select_upstream_assets(&context, &assets).expect("select upstream assets");

        assert_eq!(selected.catalog_entries.len(), 1);
        assert_eq!(selected.asset_summaries.len(), 2);
        assert_eq!(
            selected.asset_summaries[0].upstream_evidence_id,
            selected.asset_summaries[1].upstream_evidence_id
        );
        assert_eq!(
            selected.asset_summaries[0].disposition,
            VaultAssetDisposition::Acquirable
        );
        assert!(selected.asset_summaries[0].upstream_evidence_id.is_some());
        assert_ne!(
            selected.asset_summaries[0].asset_id,
            selected.asset_summaries[1].asset_id
        );
    }

    #[test]
    fn taxpayer_shaped_assets_are_metadata_only_and_never_require_vault_bytes() {
        let fixture = fixture(1, 0);
        let context = BuildContext::load(
            &fixture.root,
            &fixture.ledger_path,
            &fixture.catalog_path,
            ReadScope::External,
        )
        .expect("load build context");
        let assets = vec![
            ManifestAsset {
                asset_id: "package-7.9.6".to_owned(),
                kind: "official-package-executable".to_owned(),
                sha256: fixture.upstream_hash.clone(),
                size_bytes: fixture.upstream_bytes.len() as u64,
            },
            ManifestAsset {
                asset_id: "dummy-final-copy".to_owned(),
                kind: "dummy-profile-encrypted-final-copy".to_owned(),
                sha256: "f".repeat(64),
                size_bytes: 4096,
            },
        ];

        let selected = select_upstream_assets(&context, &assets)
            .expect("metadata-only payload must not require catalog bytes");
        assert_eq!(selected.catalog_entries.len(), 1);
        assert_eq!(selected.asset_summaries.len(), 2);
        let payload = selected
            .asset_summaries
            .iter()
            .find(|summary| summary.asset_id == "dummy-final-copy")
            .expect("metadata-only asset summary");
        assert_eq!(
            payload.disposition,
            VaultAssetDisposition::MetadataOnlyTaxpayerPayload
        );
        assert_eq!(payload.upstream_evidence_id, None);
        assert_eq!(payload.sha256, "f".repeat(64));
        assert_eq!(payload.size_bytes, 4096);

        let ledger = context
            .ledger_entry("form-001")
            .expect("fixture ledger entry");
        let gaps = build_gap_summaries(
            ledger,
            &assets,
            &[format!("sha256-{}", fixture.upstream_hash)],
        )
        .expect("metadata-only gap projection");
        let gap = gaps
            .iter()
            .find(|gap| gap.gap_id == "metadata-only-taxpayer-payload-dummy-final-copy")
            .expect("metadata-only payload gap");
        assert!(gap.source_evidence_ids.is_empty());
        assert!(gap.reason.contains("sha256="));
        assert!(gap.reason.contains("size_bytes=4096"));
    }

    #[test]
    fn serialization_count_delta_is_an_explicit_non_observed_gap() {
        let fixture = fixture(1, 1);
        let context = BuildContext::load(
            &fixture.root,
            &fixture.ledger_path,
            &fixture.catalog_path,
            ReadScope::External,
        )
        .expect("load build context");
        let index = &context.rules_index.forms[0];
        let ledger = context.ledger_entry(&index.form_id).expect("ledger entry");
        let plan = build_packet_plan(&context, index, ledger, false).expect("plan packet");
        let summary: Value =
            serde_json::from_slice(plan.files.get(SUMMARY_PATH).expect("derived summary bytes"))
                .expect("parse derived summary");
        assert_eq!(
            summary.pointer("/xml_inventory/basis"),
            Some(&json!("field-key-projection"))
        );
        assert_eq!(
            summary.pointer("/xml_inventory/projected_field_key_count"),
            Some(&json!(1))
        );
        assert_eq!(
            summary.pointer("/xml_inventory/declared_serializable_count"),
            Some(&json!(2))
        );
        assert_eq!(
            summary.pointer("/xml_inventory/unresolved_count_delta"),
            Some(&json!(1))
        );
        assert_eq!(
            summary.pointer("/xml_inventory/records/0/observed"),
            Some(&json!(false))
        );
        assert_eq!(
            summary.pointer("/xml_inventory/records/0/occurrence"),
            Some(&Value::Null)
        );
        let bytes = plan.files.get(SUMMARY_PATH).expect("summary");
        let text = std::str::from_utf8(bytes).expect("summary UTF-8");
        assert!(!text.contains("default_value"));
        assert!(!text.contains("exact_message"));
        assert!(!text.contains("C:\\"));
        assert!(text.contains("serialization-occurrences-not-observed"));
    }

    #[test]
    fn exact_43_packet_set_is_byte_stable_and_vault_checkable() {
        let mut fixture = fixture(43, 0);
        let mut dry_options = BuildEvidencePacketSetOptions::external_workspace(
            &fixture.root,
            &fixture.ledger_path,
            &fixture.catalog_path,
            fixture.root.join("dry-set"),
        );
        dry_options.dry_run = true;
        let dry_report = build_evidence_packet_set(&dry_options).expect("plan exact packet set");
        assert_eq!(dry_report.packet_count, 43);
        assert_eq!(dry_report.packets.len(), 43);
        assert_eq!(dry_report.packets[0].ordinal, 1);
        assert_eq!(dry_report.packets[0].form_id, "form-001");
        assert_eq!(dry_report.packets[42].ordinal, 43);
        assert_eq!(dry_report.packets[42].form_id, "form-043");
        assert!(!dry_report.written);
        assert!(!dry_options.output_root.exists());

        bind_expected_digests(&mut fixture);
        let first = fixture.root.join("set-a");
        let second = fixture.root.join("set-b");
        let report_a =
            build_evidence_packet_set(&BuildEvidencePacketSetOptions::external_workspace(
                &fixture.root,
                &fixture.ledger_path,
                &fixture.catalog_path,
                &first,
            ))
            .expect("build first set");
        let _report_b =
            build_evidence_packet_set(&BuildEvidencePacketSetOptions::external_workspace(
                &fixture.root,
                &fixture.ledger_path,
                &fixture.catalog_path,
                &second,
            ))
            .expect("build second set");
        assert_eq!(report_a.packet_count, 43);
        assert_eq!(
            report_a.packet_set_digest_sha256,
            dry_report.packet_set_digest_sha256
        );
        assert_eq!(report_a.packets, dry_report.packets);
        assert_eq!(
            read_external_tree(&first).expect("first set"),
            read_external_tree(&second).expect("second set")
        );
        let alias_component = fixture.root.join("packet-root-alias-component");
        fs::create_dir(&alias_component).expect("create packet-root alias component");
        let noncanonical_first = alias_component.join("..").join("set-a");
        assert_eq!(
            check_packet_set_at(
                &fixture.root,
                &noncanonical_first,
                None,
                ReadScope::External,
            )
            .expect("check packet set through non-canonical root spelling")
            .packet_set_digest_sha256,
            report_a.packet_set_digest_sha256
        );
        let report = check_evidence_packet_set(&CheckEvidencePacketSetOptions::external_workspace(
            &fixture.root,
            &first,
        ))
        .expect("check packet set without vault");
        assert_eq!(report.packet_count, 43);
        assert!(!report.full_upstream_verified);
        assert_eq!(report.packets[0].form_id, "form-001");
        assert_eq!(report.packets[42].form_id, "form-043");

        let vault = write_vault(&fixture, &fixture.root);
        let mut options = CheckEvidencePacketSetOptions::external_workspace(&fixture.root, &first);
        options.vault_dir = Some(vault.clone());
        assert!(
            check_evidence_packet_set(&options)
                .expect("check packet set with vault")
                .full_upstream_verified
        );
        let content = vault.join(
            content_addressed_path(&fixture.upstream_hash)
                .replace('/', std::path::MAIN_SEPARATOR_STR),
        );
        fs::write(&content, b"tampered").expect("tamper test vault");
        assert!(
            check_evidence_packet_set(&options)
                .expect_err("tampered vault must fail")
                .to_string()
                .contains("mismatch")
        );
    }

    #[test]
    fn aggregate_rejects_extra_missing_reordered_and_drifted_packets() {
        let mut fixture = fixture(3, 0);
        bind_expected_digests(&mut fixture);
        let output = fixture.root.join("set");
        build_evidence_packet_set(&BuildEvidencePacketSetOptions::external_workspace(
            &fixture.root,
            &fixture.ledger_path,
            &fixture.catalog_path,
            &output,
        ))
        .expect("build packet set");

        let tracked_source = fixture.root.join("rules/forms/form-001/evidence.md");
        let original_source = fs::read(&tracked_source).expect("read tracked source");
        fs::write(&tracked_source, b"# Evidence\n\nDrift.\n").expect("drift tracked source");
        let error = check_evidence_packet_set(&CheckEvidencePacketSetOptions::external_workspace(
            &fixture.root,
            &output,
        ))
        .expect_err("tracked v1 source drift must fail");
        assert!(error.to_string().contains("tracked v1 source drift"));
        fs::write(&tracked_source, original_source).expect("restore tracked source");

        fs::create_dir(output.join("extra")).expect("create extra root");
        fs::write(output.join("extra/file"), b"x").expect("write extra root file");
        let error = check_evidence_packet_set(&CheckEvidencePacketSetOptions::external_workspace(
            &fixture.root,
            &output,
        ))
        .expect_err("extra packet root entry must fail");
        assert!(error.to_string().contains("top-level bijection"));
        fs::remove_dir_all(output.join("extra")).expect("remove test extra");

        let summary_path = output.join("form-001").join(SUMMARY_PATH);
        let original_summary = fs::read(&summary_path).expect("read summary");
        let mut drifted = original_summary.clone();
        drifted.push(b' ');
        fs::write(&summary_path, drifted).expect("drift summary");
        assert!(
            check_evidence_packet_set(&CheckEvidencePacketSetOptions::external_workspace(
                &fixture.root,
                &output,
            ))
            .expect_err("derived drift must fail")
            .to_string()
            .contains("mismatch")
        );
        fs::write(&summary_path, original_summary).expect("restore summary");

        let set_path = output.join(EVIDENCE_PACKET_SET_MANIFEST);
        let original_set = fs::read(&set_path).expect("read set manifest");
        let mut set: PacketSetManifest =
            serde_json::from_slice(&original_set).expect("parse set manifest");
        set.packets.swap(0, 1);
        set.packet_set_digest_sha256 = packet_set_digest(&set).expect("recompute reordered digest");
        write_typed_json(&set_path, &set);
        assert!(
            check_evidence_packet_set(&CheckEvidencePacketSetOptions::external_workspace(
                &fixture.root,
                &output,
            ))
            .expect_err("reordered set must fail")
            .to_string()
            .contains("order/bijection")
        );
        fs::write(&set_path, original_set).expect("restore set manifest");

        fs::remove_dir_all(output.join("form-003")).expect("remove one test packet");
        assert!(
            check_evidence_packet_set(&CheckEvidencePacketSetOptions::external_workspace(
                &fixture.root,
                &output,
            ))
            .expect_err("missing packet must fail")
            .to_string()
            .contains("top-level bijection")
        );
    }

    #[test]
    fn aggregate_rejects_a_digest_consistent_candidate_packet() {
        let mut fixture = fixture(1, 0);
        bind_expected_digests(&mut fixture);
        let output = fixture.root.join("set");
        build_evidence_packet_set(&BuildEvidencePacketSetOptions::external_workspace(
            &fixture.root,
            &fixture.ledger_path,
            &fixture.catalog_path,
            &output,
        ))
        .expect("build reviewed packet set");

        let packet_root = output.join("form-001");
        let manifest_path = packet_root.join(super::EVIDENCE_PACKET_MANIFEST);
        let mut manifest: EvidencePacketManifest =
            serde_json::from_slice(&fs::read(&manifest_path).expect("read packet manifest"))
                .expect("parse packet manifest");
        manifest.review = EvidenceReview {
            status: EvidenceReviewStatus::Candidate,
            reviewed_by: None,
            reviewed_at_utc: None,
        };
        for file in &mut manifest.derived_evidence {
            file.review_status = EvidenceReviewStatus::Candidate;
        }
        let mut derived = read_external_tree(&packet_root).expect("read packet tree");
        derived.remove(super::EVIDENCE_PACKET_MANIFEST);
        manifest.packet_digest_sha256 =
            evidence_packet_digest(&manifest, &derived).expect("recompute candidate digest");
        let manifest_bytes =
            canonical_serialize(&manifest, "candidate packet manifest").expect("serialize");
        fs::write(&manifest_path, &manifest_bytes).expect("write candidate manifest");

        let set_path = output.join(EVIDENCE_PACKET_SET_MANIFEST);
        let mut set: PacketSetManifest =
            serde_json::from_slice(&fs::read(&set_path).expect("read set manifest"))
                .expect("parse set manifest");
        set.packets[0].packet_digest_sha256 = manifest.packet_digest_sha256;
        set.packets[0].manifest_sha256 = sha256_hex(&manifest_bytes);
        set.packet_set_digest_sha256 = packet_set_digest(&set).expect("recompute set digest");
        write_typed_json(&set_path, &set);

        let error = check_evidence_packet_set(&CheckEvidencePacketSetOptions::external_workspace(
            &fixture.root,
            &output,
        ))
        .expect_err("aggregate checker must reject candidate packet");
        assert!(error.to_string().contains("must be explicitly reviewed"));
    }

    #[test]
    fn fresh_output_failure_leaves_partial_target_without_path_cleanup() {
        let root = temporary_directory("fail-closed-residue");
        let target = root.join("output");
        let files = BTreeMap::from([
            ("0/nested/file".to_owned(), b"nested".to_vec()),
            ("a".to_owned(), b"file".to_vec()),
            ("a/b".to_owned(), b"conflict".to_vec()),
        ]);
        let error = write_fresh_fail_closed(&target, &files, |_| Ok(()))
            .expect_err("path conflict must fail");
        assert!(
            error.to_string().contains("left in place"),
            "failure must identify the fail-closed residue: {error}"
        );
        assert_eq!(
            fs::read(target.join("0/nested/file")).expect("read preserved first file"),
            b"nested"
        );
        assert_eq!(
            fs::read(target.join("a")).expect("read preserved conflicting file"),
            b"file"
        );
        fs::remove_dir_all(&root).expect("remove test-owned residue");
    }

    #[test]
    fn canonical_rules_target_and_non_normalized_alias_are_rejected() {
        let fixture = fixture(1, 0);
        let rules_target = fixture.root.join("rules/new-packet-output");
        let error = reject_canonical_rules_target(&fixture.root, &rules_target)
            .expect_err("output under canonical rules must fail");
        assert!(error.to_string().contains("canonical rules"));

        let non_normalized = fixture.root.join("outside/../rules/new-packet-output");
        let error = reject_canonical_rules_target(&fixture.root, &non_normalized)
            .expect_err("non-normalized alias must fail closed");
        assert!(error.to_string().contains("lexically normalized"));
    }

    #[cfg(unix)]
    #[test]
    fn output_rejects_symlink_ancestor() {
        use std::os::unix::fs::symlink;

        let root = temporary_directory("symlink");
        let real = root.join("real");
        fs::create_dir(&real).expect("create real parent");
        let link = root.join("link");
        symlink(&real, &link).expect("create test symlink");
        let target = link.join("output");
        let files = BTreeMap::from([("file".to_owned(), b"bytes".to_vec())]);
        let error =
            write_fresh_fail_closed(&target, &files, |_| Ok(())).expect_err("symlink must fail");
        assert!(error.to_string().contains("symlink"));
        fs::remove_file(&link).expect("remove test symlink");
        fs::remove_dir_all(&root).expect("remove symlink test root");
    }

    #[cfg(unix)]
    #[test]
    fn review_ledger_symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let fixture = fixture(1, 0);
        let link = fixture.root.join("review/ledger-link.json");
        symlink(&fixture.ledger_path, &link).expect("create ledger symlink");
        let mut options = BuildEvidencePacketOptions::external_workspace(
            &fixture.root,
            "form-001",
            &link,
            &fixture.catalog_path,
            fixture.root.join("dry"),
        );
        options.dry_run = true;
        let error = build_evidence_packet(&options).expect_err("ledger symlink must fail");
        assert!(error.to_string().contains("symlink/reparse"));
    }
}
