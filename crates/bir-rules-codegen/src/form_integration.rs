//! Deterministic, add-only integration of one packet-backed v2 form workspace.
//!
//! This module deliberately does not generate into `bir-rules` and does not
//! touch application code. It constructs the complete proposed
//! `rules/ir/v2` tree in memory, audits and deterministically generates from a
//! tool-owned external proposal repository. In-process apply is deliberately
//! unavailable because portable filesystem APIs cannot atomically
//! compare-and-swap a directory against non-cooperating writers. Existing
//! snapshots are immutable through this entrypoint; promotion and filing-safe
//! execution remain separate reviewed transactions.

use std::collections::{BTreeMap, BTreeSet, hash_map::RandomState};
use std::fs::{self, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use same_file::Handle;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audit::{AuditOptions, AuditReport, audit};
use crate::error::{CodegenError, Result};
use crate::evidence::{
    EVIDENCE_PACKET_DIGEST_DOMAIN, EVIDENCE_PACKET_MANIFEST, EvidenceAttestation,
    EvidenceCaptureProvenance, EvidencePacketManifest, EvidenceReview, EvidenceReviewStatus,
    RuleSetSourceState, VerifyEvidenceOptions, verify_evidence,
};
use crate::evidence_set::{
    EVIDENCE_REVIEW_LEDGER_FORMAT, EVIDENCE_SUMMARY_FORMAT, TRACKED_V1_SOURCE_SET_DOMAIN,
};
use crate::files::{read_bytes, read_tree, write_tree_atomically};
use crate::generate::{GenerationReport, build_generated_files};
use crate::hash::{digest_entries, sha256_hex};
use crate::json::{CANONICALIZATION_ID, canonical_bytes, parse_strict};
use crate::model::{BranchState, ReviewStatus, SerializationArtifactBranch};
use crate::path::{
    DEFAULT_SCHEMA_DIR, DEFAULT_SOURCE_DIR, canonical_repo_root, is_same_or_below,
    is_symlink_or_reparse_point, resolve_existing_under, validate_portable_relative,
};
use crate::verified_file::open_verified_regular_file;

pub const FORM_INTEGRATION_TREE_DIGEST_DOMAIN: &str = "bir-rules-form-integration-tree-v1";
pub const PACKET_BACKED_HANDOFF_FORMAT: &str = "bir-packet-backed-form-handoff-v1";
pub const PROTECTED_2550Q_RULE_SET_ID: &str = "2550q-v2024-p7.9.6.0";

const INDEX_PATH: &str = "index.json";
const HANDOFF_PATH: &str = "HANDOFF.json";
const HANDOFF_MARKDOWN_PATH: &str = "HANDOFF.md";
const PROPOSAL_PREFIX: &str = "bir-rules-codegen-form-integration-proposal";
const PROPOSAL_OWNER_FILE: &str = "OWNER";
const TRACKED_V1_SUMMARY_PATH: &str = "derived/tracked-v1-summary.json";

static PROPOSAL_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Options for one add-only canonical v2 corpus transaction.
///
/// Construction defaults to dry-run. [`Self::with_apply`] records an explicit
/// request, but integration fails closed because no portable atomic directory
/// compare-and-swap is available.
#[derive(Clone, Debug)]
pub struct FormIntegrationOptions {
    pub repo_root: PathBuf,
    pub staging_root: PathBuf,
    pub reviewed_packet_dir: PathBuf,
    pub review_ledger_path: PathBuf,
    pub rule_set_id: String,
    pub dry_run: bool,
}

impl FormIntegrationOptions {
    pub fn new(
        repo_root: impl Into<PathBuf>,
        staging_root: impl Into<PathBuf>,
        reviewed_packet_dir: impl Into<PathBuf>,
        review_ledger_path: impl Into<PathBuf>,
        rule_set_id: impl Into<String>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            staging_root: staging_root.into(),
            reviewed_packet_dir: reviewed_packet_dir.into(),
            review_ledger_path: review_ledger_path.into(),
            rule_set_id: rule_set_id.into(),
            dry_run: true,
        }
    }

    pub fn with_apply(mut self) -> Self {
        self.dry_run = false;
        self
    }
}

/// Hash-bound identity of one exact proposed v2 source file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FormIntegrationFile {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

/// Complete, deterministic report for the current/staged/proposed comparison.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FormIntegrationReport {
    pub rule_set_id: String,
    pub review_status: String,
    pub canonical_source_root: PathBuf,
    pub staging_source_root: PathBuf,
    pub current_snapshot_count: usize,
    pub staged_snapshot_count: usize,
    pub proposed_snapshot_count: usize,
    pub current_tree_sha256: String,
    pub staged_tree_sha256: String,
    pub proposed_tree_sha256: String,
    /// Full canonical `rules/` context excluding `rules/ir/v2`, binding the
    /// exact repository context against which this dry-run proposal validated.
    pub canonical_context_sha256: String,
    pub current_audit_sha256: String,
    pub staged_audit_sha256: String,
    pub proposed_audit_sha256: String,
    pub generated_output_sha256: String,
    pub generated_manifest_sha256: String,
    /// Every file in the proposed `rules/ir/v2` tree, in portable path order.
    pub proposed_files: Vec<FormIntegrationFile>,
    /// The exact subset whose bytes differ from the current tree.
    pub changed_files: Vec<FormIntegrationFile>,
    pub applied: bool,
}

/// Command-oriented aliases matching the crate's other `*Options` reports.
pub type IntegrateFormOptions = FormIntegrationOptions;
pub type IntegrateFormReport = FormIntegrationReport;
pub type IntegrateFormFile = FormIntegrationFile;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ClosedIndex {
    #[serde(rename = "$schema")]
    schema: String,
    schema_version: String,
    snapshots: Vec<ClosedIndexSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ClosedIndexSnapshot {
    rule_set_id: String,
    form_code: String,
    form_revision: String,
    official_package_version: String,
    source_set_sha256: Option<String>,
    path: String,
    review_status: ClosedReviewStatus,
    profile_states: ClosedProfileStates,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ClosedReviewStatus {
    Skeleton,
    Candidate,
    Reviewed,
}

impl ClosedReviewStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Skeleton => "skeleton",
            Self::Candidate => "candidate",
            Self::Reviewed => "reviewed",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ClosedProfileStates {
    official: ClosedBranchState,
    filing_safe: ClosedBranchState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ClosedBranchState {
    Executable,
    DocumentedOnly,
    Unresolved,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PacketBackedHandoff {
    format: String,
    canonicalization: String,
    packet: HandoffPacket,
    identity: HandoffIdentity,
    review_status: String,
    canonical_integration_performed: bool,
    proves_executable_semantics: bool,
    legacy_record_census: Value,
    serialization_occurrences: Value,
    blocking_gaps: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HandoffPacket {
    packet_id: String,
    packet_digest_sha256: String,
    review_status: String,
    record_census_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HandoffIdentity {
    form_id: String,
    form_code: String,
    form_revision: String,
    official_package_version: String,
    rule_set_id: String,
    source_set_sha256: Option<String>,
    tracked_v1_source_set_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ClosedReviewLedger {
    format: String,
    canonicalization: String,
    entries: Vec<ClosedReviewLedgerEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ClosedReviewLedgerEntry {
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
    derived_reviews: Vec<ClosedDerivedReview>,
    source_excerpts: Vec<ClosedReviewedSourceExcerpt>,
    capture_gaps: Vec<ClosedReviewedCaptureGap>,
    expected_packet_digest_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ClosedDerivedReview {
    path: String,
    status: EvidenceReviewStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ClosedReviewedSourceExcerpt {
    excerpt_id: String,
    upstream_evidence_id: String,
    excerpt_start_byte: u64,
    excerpt_end_byte: u64,
    excerpt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ClosedReviewedCaptureGap {
    gap_id: String,
    reason: String,
    source_evidence_ids: Vec<String>,
}

struct ReviewedArtifactBinding {
    manifest: EvidencePacketManifest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InventoryEntryKind {
    File,
    Directory,
}

struct CapturedStagingWorkspace {
    workspace_tree: BTreeMap<String, Vec<u8>>,
    form_tree: BTreeMap<String, Vec<u8>>,
    schema_tree: BTreeMap<String, Vec<u8>>,
    v2_tree: BTreeMap<String, Vec<u8>>,
    handoff_bytes: Vec<u8>,
}

struct IntegrationPlan {
    source_root: PathBuf,
    current_tree: BTreeMap<String, Vec<u8>>,
    report: FormIntegrationReport,
}

/// Builds and validates one add-only form integration proposal.
///
/// Apply requests fail closed because the filesystem API cannot provide the
/// required atomic directory compare-and-swap.
///
/// All three corpora are audited without a rule-set filter:
///
/// - the complete current canonical corpus;
/// - the complete (exactly one snapshot) staged workspace; and
/// - the complete proposed aggregate corpus.
///
/// Generation is built twice in memory from two independent proposal audits.
/// No generated or application path is written.
pub fn integrate_form(options: &FormIntegrationOptions) -> Result<FormIntegrationReport> {
    let plan = build_integration_plan(options)?;
    if !options.dry_run {
        return refuse_non_atomic_apply(&plan.source_root, &plan.current_tree);
    }
    Ok(plan.report)
}

fn build_integration_plan(options: &FormIntegrationOptions) -> Result<IntegrationPlan> {
    validate_rule_set_id(&options.rule_set_id)?;
    if options.rule_set_id == PROTECTED_2550Q_RULE_SET_ID {
        return Err(CodegenError::new(format!(
            "rule set `{PROTECTED_2550Q_RULE_SET_ID}` is protected and cannot be integrated or replaced by this command"
        )));
    }

    let repo_root = canonical_repo_root(&options.repo_root)?;
    let staging_root = canonical_external_directory(
        &options.staging_root,
        &repo_root,
        "packet-backed staging root",
    )?;
    let reviewed_packet_root = canonical_external_directory(
        &options.reviewed_packet_dir,
        &repo_root,
        "reviewed evidence packet",
    )?;
    require_disjoint_directories(
        &staging_root,
        "packet-backed staging root",
        &reviewed_packet_root,
        "reviewed evidence packet",
    )?;
    let review_ledger_path =
        canonical_external_file(&options.review_ledger_path, &repo_root, "review ledger")?;
    require_file_outside_directory(
        &review_ledger_path,
        "review ledger",
        &staging_root,
        "packet-backed staging root",
    )?;
    require_file_outside_directory(
        &review_ledger_path,
        "review ledger",
        &reviewed_packet_root,
        "reviewed evidence packet",
    )?;
    let reviewed_artifacts =
        load_reviewed_artifact_binding(&reviewed_packet_root, &review_ledger_path)?;
    if reviewed_artifacts.manifest.rule_set_id != options.rule_set_id {
        return Err(CodegenError::new(format!(
            "reviewed packet rule_set_id `{}` differs from requested `{}`",
            reviewed_artifacts.manifest.rule_set_id, options.rule_set_id
        )));
    }

    let rules_root = resolve_existing_under(&repo_root, "rules", "canonical rules root")?;
    let source_root =
        resolve_existing_under(&repo_root, DEFAULT_SOURCE_DIR, "canonical v2 source root")?;
    let canonical_schema_root =
        resolve_existing_under(&repo_root, DEFAULT_SCHEMA_DIR, "canonical v2 schema root")?;
    let form_id = reviewed_artifacts.manifest.form_id.as_str();
    let form_id_components = validate_portable_relative(form_id, "reviewed packet form_id")?;
    if form_id_components.len() != 1 {
        return Err(CodegenError::new(format!(
            "reviewed packet form_id `{form_id}` must be one portable path component"
        )));
    }
    let canonical_form_relative = format!("rules/forms/{form_id}");
    let canonical_form_root = resolve_existing_under(
        &repo_root,
        &canonical_form_relative,
        "canonical v1 form root",
    )?;
    let canonical_form_tree = read_tree(&canonical_form_root)?;
    let canonical_schemas = read_tree(&canonical_schema_root)?;
    let current_tree = read_tree(&source_root)?;

    // Inventory names and entry types across the external workspace before
    // opening any staged file. Only the exact fixed handoff/form/schema/v2
    // paths are then read, and a second inventory check closes add/swap races.
    let captured_staging = capture_staging_workspace(
        &staging_root,
        form_id,
        &options.rule_set_id,
        &canonical_form_tree,
        &canonical_schemas,
    )?;
    let staging_source_root =
        resolve_existing_under(&staging_root, DEFAULT_SOURCE_DIR, "staged v2 source root")?;
    let handoff = load_handoff(
        &captured_staging.handoff_bytes,
        &staging_root.join(HANDOFF_PATH),
    )?;
    compare_trees(
        "staged packet-backed v1 form mirror",
        &canonical_form_tree,
        &captured_staging.form_tree,
    )?;
    let tracked_v1_source_set_sha256 =
        tracked_v1_source_set_digest(&canonical_form_root, &canonical_form_tree)?;
    validate_v1_manifest_identity(
        &canonical_form_root,
        &canonical_form_tree,
        &reviewed_artifacts.manifest,
    )?;
    if reviewed_artifacts.manifest.tracked_v1_source_set_sha256 != tracked_v1_source_set_sha256 {
        return Err(CodegenError::new(format!(
            "reviewed packet tracked-v1 digest {} differs from recomputed canonical digest {tracked_v1_source_set_sha256}",
            reviewed_artifacts.manifest.tracked_v1_source_set_sha256
        )));
    }

    compare_trees(
        "staged v2 schemas",
        &canonical_schemas,
        &captured_staging.schema_tree,
    )?;

    let staged_tree = captured_staging.v2_tree.clone();
    reject_existing_canonical_target(&source_root, &options.rule_set_id)?;
    require_json_only_tree("canonical v2 source tree", &current_tree)?;
    require_json_only_tree("staged v2 source tree", &staged_tree)?;
    require_single_staged_directory(&staged_tree, &options.rule_set_id)?;
    reject_case_fold_collisions("canonical v2 source tree", current_tree.keys())?;
    reject_case_fold_collisions("staged v2 source tree", staged_tree.keys())?;

    let current_index = parse_index(
        required_tree_file(&current_tree, INDEX_PATH, "canonical v2 source tree")?,
        &source_root.join(INDEX_PATH),
    )?;
    let staged_index = parse_index(
        required_tree_file(&staged_tree, INDEX_PATH, "staged v2 source tree")?,
        &staging_source_root.join(INDEX_PATH),
    )?;
    let [staged_index_snapshot] = staged_index.snapshots.as_slice() else {
        return Err(CodegenError::new(format!(
            "staged v2 index must contain exactly one snapshot, found {}",
            staged_index.snapshots.len()
        )));
    };
    if staged_index_snapshot.rule_set_id != options.rule_set_id {
        return Err(CodegenError::new(format!(
            "staged v2 index contains rule_set_id `{}`, expected `{}`",
            staged_index_snapshot.rule_set_id, options.rule_set_id
        )));
    }
    validate_closed_snapshot_identity(staged_index_snapshot, &reviewed_artifacts.manifest)?;
    require_allowed_review_state(staged_index_snapshot)?;
    if staged_index_snapshot.profile_states.filing_safe != ClosedBranchState::Unresolved {
        return Err(CodegenError::new(format!(
            "staged rule set `{}` does not keep filing_safe unresolved; canonical integration is library-only",
            options.rule_set_id
        )));
    }
    if current_index
        .snapshots
        .iter()
        .any(|snapshot| snapshot.rule_set_id == options.rule_set_id)
    {
        return Err(CodegenError::new(format!(
            "canonical v2 index already contains rule_set_id `{}`; this command is add-only and refuses overwrite",
            options.rule_set_id
        )));
    }
    if let Some(existing) = current_index.snapshots.iter().find(|snapshot| {
        snapshot.form_code == staged_index_snapshot.form_code
            && snapshot.form_revision == staged_index_snapshot.form_revision
            && snapshot.official_package_version == staged_index_snapshot.official_package_version
    }) {
        return Err(CodegenError::new(format!(
            "staged form/revision/package identity collides with canonical rule set `{}`",
            existing.rule_set_id
        )));
    }

    let protected_index = current_index
        .snapshots
        .iter()
        .find(|snapshot| snapshot.rule_set_id == PROTECTED_2550Q_RULE_SET_ID)
        .cloned()
        .ok_or_else(|| {
            CodegenError::new(format!(
                "canonical v2 index is missing protected rule set `{PROTECTED_2550Q_RULE_SET_ID}`"
            ))
        })?;
    require_protected_tree(&current_tree)?;

    let current_audit = audit(&AuditOptions::new(&repo_root))?;
    require_protected_audit(&current_audit, &protected_index)?;
    if current_audit.snapshot_count() != current_index.snapshots.len() {
        return Err(CodegenError::new(
            "canonical index/audit snapshot counts differ before integration",
        ));
    }

    let staged_audit = audit_captured_staging(&staging_root, &captured_staging.workspace_tree)?;
    if staged_audit.snapshot_count() != 1 {
        return Err(CodegenError::new(format!(
            "staged aggregate audit must retain exactly one snapshot, found {}",
            staged_audit.snapshot_count()
        )));
    }
    if staged_audit.schema_digest() != current_audit.schema_digest() {
        return Err(CodegenError::new(
            "staged and canonical aggregate audits resolved different v2 schema sets",
        ));
    }
    let staged_snapshot = staged_audit
        .snapshots
        .first()
        .expect("one staged snapshot was required");
    if staged_snapshot.index.rule_set_id != options.rule_set_id {
        return Err(CodegenError::new(format!(
            "staged aggregate audit resolved `{}`, expected `{}`",
            staged_snapshot.index.rule_set_id, options.rule_set_id
        )));
    }
    require_staged_snapshot_safe(staged_snapshot)?;
    validate_handoff(
        &handoff,
        staged_snapshot,
        &reviewed_artifacts.manifest,
        &tracked_v1_source_set_sha256,
    )?;

    let mut proposed_index = current_index.clone();
    proposed_index.snapshots.push(staged_index_snapshot.clone());
    proposed_index
        .snapshots
        .sort_by(|left, right| left.rule_set_id.cmp(&right.rule_set_id));

    let mut proposed_tree = current_tree.clone();
    for (path, bytes) in &staged_tree {
        if path == INDEX_PATH {
            continue;
        }
        if proposed_tree.insert(path.clone(), bytes.clone()).is_some() {
            return Err(CodegenError::new(format!(
                "staged v2 file `{path}` collides with an existing canonical file"
            )));
        }
    }
    proposed_tree.insert(INDEX_PATH.to_owned(), render_index(&proposed_index)?);
    reject_case_fold_collisions("proposed v2 source tree", proposed_tree.keys())?;
    require_add_only_delta(
        &current_tree,
        &staged_tree,
        &proposed_tree,
        &options.rule_set_id,
    )?;
    require_protected_tree_unchanged(&current_tree, &proposed_tree)?;
    let proposed_protected = proposed_index
        .snapshots
        .iter()
        .find(|snapshot| snapshot.rule_set_id == PROTECTED_2550Q_RULE_SET_ID)
        .expect("protected snapshot was retained by add-only index merge");
    if proposed_protected != &protected_index {
        return Err(CodegenError::new(
            "proposed index changes the protected 2550Q snapshot",
        ));
    }

    let (proposed_audit, generation, canonical_context_sha256) = validate_external_proposal(
        &repo_root,
        &staging_root,
        &rules_root,
        &current_tree,
        &proposed_tree,
        &options.rule_set_id,
        &protected_index,
    )?;
    if proposed_audit.snapshot_count() != current_audit.snapshot_count() + 1 {
        return Err(CodegenError::new(format!(
            "proposed aggregate audit changed snapshot count from {} to {}; exactly one addition is required",
            current_audit.snapshot_count(),
            proposed_audit.snapshot_count()
        )));
    }
    if proposed_audit.schema_digest() != current_audit.schema_digest() {
        return Err(CodegenError::new(
            "proposed aggregate audit did not retain the canonical v2 schema digest",
        ));
    }
    if generation.reviewed_snapshot_count
        != current_audit
            .snapshots
            .iter()
            .filter(|snapshot| snapshot.document.review_status == ReviewStatus::Reviewed)
            .count()
    {
        return Err(CodegenError::new(
            "proposed generation changes reviewed snapshot count; promotion is forbidden",
        ));
    }
    let final_staging = capture_staging_workspace(
        &staging_root,
        form_id,
        &options.rule_set_id,
        &canonical_form_tree,
        &canonical_schemas,
    )?;
    compare_trees(
        "packet-backed staging workspace final digest recheck",
        &captured_staging.workspace_tree,
        &final_staging.workspace_tree,
    )?;

    let current_tree_sha256 = tree_digest(&current_tree);
    let staged_tree_sha256 = tree_digest(&staged_tree);
    let proposed_tree_sha256 = tree_digest(&proposed_tree);
    let proposed_files = file_manifest(&proposed_tree);
    let changed_tree = proposed_tree
        .iter()
        .filter_map(|(path, bytes)| {
            (current_tree.get(path) != Some(bytes)).then(|| (path.clone(), bytes.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let changed_files = file_manifest(&changed_tree);
    let report = FormIntegrationReport {
        rule_set_id: options.rule_set_id.clone(),
        review_status: staged_index_snapshot.review_status.as_str().to_owned(),
        canonical_source_root: source_root.clone(),
        staging_source_root: staging_source_root.clone(),
        current_snapshot_count: current_audit.snapshot_count(),
        staged_snapshot_count: staged_audit.snapshot_count(),
        proposed_snapshot_count: proposed_audit.snapshot_count(),
        current_tree_sha256,
        staged_tree_sha256,
        proposed_tree_sha256,
        canonical_context_sha256: canonical_context_sha256.clone(),
        current_audit_sha256: current_audit.normalized_source_digest().to_owned(),
        staged_audit_sha256: staged_audit.normalized_source_digest().to_owned(),
        proposed_audit_sha256: proposed_audit.normalized_source_digest().to_owned(),
        generated_output_sha256: generation.generated_output_digest,
        generated_manifest_sha256: generation.manifest_digest,
        proposed_files,
        changed_files,
        applied: false,
    };

    Ok(IntegrationPlan {
        source_root,
        current_tree,
        report,
    })
}

fn refuse_non_atomic_apply<T>(
    source_root: &Path,
    expected_tree: &BTreeMap<String, Vec<u8>>,
) -> Result<T> {
    let live_tree = read_tree(source_root)?;
    if &live_tree != expected_tree {
        return Err(CodegenError::new(format!(
            "canonical v2 source tree changed outside form integration (expected {}, found {}); no bytes were written",
            tree_digest(expected_tree),
            tree_digest(&live_tree)
        )));
    }
    Err(CodegenError::new(
        "form integration apply is unavailable: this filesystem API cannot provide an atomic compare-and-swap for a non-cooperatively modified directory; use dry-run and a separately reviewed atomic evidence transaction",
    ))
}

fn validate_external_proposal(
    repo_root: &Path,
    staging_root: &Path,
    rules_root: &Path,
    expected_current_tree: &BTreeMap<String, Vec<u8>>,
    proposed_tree: &BTreeMap<String, Vec<u8>>,
    rule_set_id: &str,
    protected_index: &ClosedIndexSnapshot,
) -> Result<(AuditReport, GenerationReport, String)> {
    let canonical_rules_tree = read_tree(rules_root)?;
    let extracted_current = extract_subtree(&canonical_rules_tree, "ir/v2/");
    compare_trees(
        "canonical rules tree during proposal construction",
        expected_current_tree,
        &extracted_current,
    )?;
    let canonical_context_sha256 = tree_digest(&without_subtree(&canonical_rules_tree, "ir/v2/"));

    let rustfmt_path = repo_root.join(".rustfmt.toml");
    let rustfmt_metadata = fs::symlink_metadata(&rustfmt_path).map_err(|source| {
        CodegenError::io("inspect canonical rustfmt config", &rustfmt_path, source)
    })?;
    if is_symlink_or_reparse_point(&rustfmt_metadata) || !rustfmt_metadata.is_file() {
        return Err(CodegenError::new(format!(
            "canonical rustfmt config `{}` must be a real file",
            rustfmt_path.display()
        )));
    }

    let mut proposal_files = BTreeMap::new();
    proposal_files.insert(".rustfmt.toml".to_owned(), read_bytes(&rustfmt_path)?);
    for (path, bytes) in canonical_rules_tree {
        if path == "ir/v2" || path.starts_with("ir/v2/") {
            continue;
        }
        proposal_files.insert(format!("rules/{path}"), bytes);
    }
    for (path, bytes) in proposed_tree {
        proposal_files.insert(format!("rules/ir/v2/{path}"), bytes.clone());
    }

    let mut workspace = ProposalWorkspace::create(staging_root)?;
    let validation = (|| {
        write_tree_atomically(workspace.repo_root(), &proposal_files)?;

        let first_audit = audit(&AuditOptions::new(workspace.repo_root()))?;
        first_audit.require_rule_set(rule_set_id)?;
        require_protected_audit(&first_audit, protected_index)?;
        let first_generation = build_generated_files(&first_audit)?;

        let second_audit = audit(&AuditOptions::new(workspace.repo_root()))?;
        second_audit.require_rule_set(rule_set_id)?;
        require_protected_audit(&second_audit, protected_index)?;
        let second_generation = build_generated_files(&second_audit)?;
        if first_audit.normalized_source_digest() != second_audit.normalized_source_digest()
            || first_audit.schema_digest() != second_audit.schema_digest()
            || first_generation.files != second_generation.files
            || first_generation.generated_output_digest != second_generation.generated_output_digest
            || first_generation.manifest_digest != second_generation.manifest_digest
        {
            return Err(CodegenError::new(
                "independent aggregate proposal audits/generations were not byte-deterministic",
            ));
        }
        Ok((first_audit, first_generation))
    })();
    let cleanup = workspace.cleanup();
    match (validation, cleanup) {
        (Ok((first_audit, first_generation)), Ok(())) => {
            Ok((first_audit, first_generation, canonical_context_sha256))
        }
        (Err(validation_error), Ok(())) => Err(validation_error),
        (Ok(_), Err(cleanup_error)) => Err(CodegenError::new(format!(
            "external proposal validated but cleanup failed: {cleanup_error}"
        ))),
        (Err(validation_error), Err(cleanup_error)) => Err(CodegenError::new(format!(
            "external proposal validation failed: {validation_error}; cleanup also failed: {cleanup_error}"
        ))),
    }
}

fn require_staged_snapshot_safe(snapshot: &crate::audit::AuditedSnapshot) -> Result<()> {
    if snapshot.document.review_status != ReviewStatus::Candidate {
        return Err(CodegenError::new(format!(
            "staged rule set `{}` must be candidate before canonical integration, found {:?}",
            snapshot.index.rule_set_id, snapshot.document.review_status
        )));
    }
    if snapshot.document.profile_status.filing_safe.state() != BranchState::Unresolved
        || snapshot.document.evaluation_policy.filing_safe.state() != BranchState::Unresolved
        || snapshot
            .document
            .serialization
            .artifacts
            .iter()
            .any(|artifact| {
                matches!(
                    &artifact.filing_safe,
                    SerializationArtifactBranch::Executable { .. }
                )
            })
    {
        return Err(CodegenError::new(format!(
            "staged rule set `{}` does not keep filing-safe policy unresolved or contains executable filing-safe serialization authority",
            snapshot.index.rule_set_id
        )));
    }
    reject_executable_filing_safe_json(
        &parse_strict(
            &snapshot.canonical_rule_set,
            Path::new("audited-staged-rule-set.json"),
        )?,
        "$",
    )
}

fn reject_executable_filing_safe_json(value: &crate::json::JsonValue, path: &str) -> Result<()> {
    match value {
        crate::json::JsonValue::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}.{key}");
                if key == "filing_safe"
                    && child
                        .object()
                        .and_then(|branch| branch.get("state"))
                        .and_then(crate::json::JsonValue::as_str)
                        == Some("executable")
                {
                    return Err(CodegenError::new(format!(
                        "{child_path} is executable; form integration cannot create filing-safe authority"
                    )));
                }
                reject_executable_filing_safe_json(child, &child_path)?;
            }
        }
        crate::json::JsonValue::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                reject_executable_filing_safe_json(child, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn capture_staging_workspace(
    staging_root: &Path,
    form_id: &str,
    rule_set_id: &str,
    canonical_form_tree: &BTreeMap<String, Vec<u8>>,
    canonical_schema_tree: &BTreeMap<String, Vec<u8>>,
) -> Result<CapturedStagingWorkspace> {
    let inventory = inventory_directory_metadata(staging_root, "packet-backed staging workspace")?;
    validate_staging_inventory(
        &inventory,
        form_id,
        rule_set_id,
        canonical_form_tree,
        canonical_schema_tree,
    )?;

    let mut workspace_tree = BTreeMap::new();
    for (relative, kind) in &inventory {
        if *kind != InventoryEntryKind::File {
            continue;
        }
        let path = staging_root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        workspace_tree.insert(
            relative.clone(),
            read_verified_regular_file(&path, "allowlisted staging file")?,
        );
    }
    let final_inventory =
        inventory_directory_metadata(staging_root, "packet-backed staging workspace after read")?;
    if final_inventory != inventory {
        return Err(CodegenError::new(
            "packet-backed staging workspace metadata changed during allowlisted capture",
        ));
    }

    let form_prefix = format!("rules/forms/{form_id}/");
    let schema_prefix = "rules/schema/v2/";
    let v2_prefix = "rules/ir/v2/";
    let handoff_bytes = workspace_tree
        .get(HANDOFF_PATH)
        .cloned()
        .ok_or_else(|| CodegenError::new("allowlisted staging capture lost HANDOFF.json"))?;
    Ok(CapturedStagingWorkspace {
        form_tree: extract_subtree(&workspace_tree, &form_prefix),
        schema_tree: extract_subtree(&workspace_tree, schema_prefix),
        v2_tree: extract_subtree(&workspace_tree, v2_prefix),
        workspace_tree,
        handoff_bytes,
    })
}

fn inventory_directory_metadata(
    root: &Path,
    label: &str,
) -> Result<BTreeMap<String, InventoryEntryKind>> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|source| CodegenError::io(&format!("inspect {label}"), root, source))?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be a real directory",
            root.display()
        )));
    }
    let root_handle = Handle::from_path(root)
        .map_err(|source| CodegenError::io(&format!("open {label} identity"), root, source))?;
    let mut inventory = BTreeMap::new();
    inventory_directory_entries(root, root, "", &mut inventory)?;
    let final_handle = Handle::from_path(root)
        .map_err(|source| CodegenError::io(&format!("reopen {label} identity"), root, source))?;
    if root_handle != final_handle {
        return Err(CodegenError::new(format!(
            "{label} `{}` was replaced during metadata inventory",
            root.display()
        )));
    }
    Ok(inventory)
}

fn inventory_directory_entries(
    safety_root: &Path,
    directory: &Path,
    prefix: &str,
    inventory: &mut BTreeMap<String, InventoryEntryKind>,
) -> Result<()> {
    let before = Handle::from_path(directory).map_err(|source| {
        CodegenError::io(
            "open staging inventory directory identity",
            directory,
            source,
        )
    })?;
    let mut entries = fs::read_dir(directory)
        .map_err(|source| CodegenError::io("read staging inventory directory", directory, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CodegenError::io("read staging inventory entry", directory, source))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry
            .file_name()
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "staging entry `{}` is not valid UTF-8",
                    entry.path().display()
                ))
            })?;
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        validate_portable_relative(&relative, "staging metadata path")?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| CodegenError::io("inspect staging inventory entry", &path, source))?;
        let kind = if is_symlink_or_reparse_point(&metadata) {
            return Err(CodegenError::new(format!(
                "staging workspace contains symlink or reparse point `{}`",
                path.display()
            )));
        } else if metadata.is_dir() {
            InventoryEntryKind::Directory
        } else if metadata.is_file() {
            InventoryEntryKind::File
        } else {
            return Err(CodegenError::new(format!(
                "staging workspace contains special entry `{}`",
                path.display()
            )));
        };
        if inventory.insert(relative.clone(), kind).is_some() {
            return Err(CodegenError::new(format!(
                "staging metadata contains duplicate portable path `{relative}`"
            )));
        }
        if kind == InventoryEntryKind::Directory {
            inventory_directory_entries(safety_root, &path, &relative, inventory)?;
        }
    }
    let after = Handle::from_path(directory).map_err(|source| {
        CodegenError::io(
            "reopen staging inventory directory identity",
            directory,
            source,
        )
    })?;
    if before != after || !is_same_or_below(safety_root, directory) {
        return Err(CodegenError::new(format!(
            "staging inventory directory `{}` changed or escaped its root",
            directory.display()
        )));
    }
    Ok(())
}

fn validate_staging_inventory(
    actual: &BTreeMap<String, InventoryEntryKind>,
    form_id: &str,
    rule_set_id: &str,
    canonical_form_tree: &BTreeMap<String, Vec<u8>>,
    canonical_schema_tree: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let mut expected = BTreeMap::from([
        (HANDOFF_PATH.to_owned(), InventoryEntryKind::File),
        (HANDOFF_MARKDOWN_PATH.to_owned(), InventoryEntryKind::File),
        ("rules".to_owned(), InventoryEntryKind::Directory),
        ("rules/forms".to_owned(), InventoryEntryKind::Directory),
        ("rules/schema".to_owned(), InventoryEntryKind::Directory),
        ("rules/ir".to_owned(), InventoryEntryKind::Directory),
    ]);
    let form_prefix = format!("rules/forms/{form_id}");
    expected.insert(form_prefix.clone(), InventoryEntryKind::Directory);
    for path in canonical_form_tree.keys() {
        add_expected_staging_file(&mut expected, &form_prefix, path)?;
    }
    let schema_prefix = "rules/schema/v2";
    expected.insert(schema_prefix.to_owned(), InventoryEntryKind::Directory);
    for path in canonical_schema_tree.keys() {
        add_expected_staging_file(&mut expected, schema_prefix, path)?;
    }
    let v2_prefix = "rules/ir/v2";
    let rule_set_prefix = format!("{v2_prefix}/{rule_set_id}");
    expected.insert(v2_prefix.to_owned(), InventoryEntryKind::Directory);
    expected.insert(rule_set_prefix.clone(), InventoryEntryKind::Directory);
    expected.insert(
        format!("{v2_prefix}/{INDEX_PATH}"),
        InventoryEntryKind::File,
    );

    let rule_set_file = format!("{rule_set_prefix}/rule-set.json");
    for (path, kind) in actual {
        if *kind != InventoryEntryKind::File || !path.starts_with(&format!("{rule_set_prefix}/")) {
            continue;
        }
        if !path.ends_with(".json") {
            return Err(CodegenError::new(format!(
                "staged v2 candidate contains non-JSON file `{path}`"
            )));
        }
        let relative = path
            .strip_prefix(&format!("{rule_set_prefix}/"))
            .expect("prefix was checked");
        add_expected_staging_file(&mut expected, &rule_set_prefix, relative)?;
    }
    if actual.get(&rule_set_file) != Some(&InventoryEntryKind::File) {
        return Err(CodegenError::new(format!(
            "staged v2 candidate is missing `{rule_set_file}`"
        )));
    }
    if actual != &expected {
        let unexpected = actual
            .keys()
            .filter(|path| !expected.contains_key(*path))
            .cloned()
            .collect::<Vec<_>>();
        let missing = expected
            .keys()
            .filter(|path| !actual.contains_key(*path))
            .cloned()
            .collect::<Vec<_>>();
        return Err(CodegenError::new(format!(
            "staging workspace differs from its closed allowlist; unexpected=[{}] missing=[{}]",
            unexpected.join(", "),
            missing.join(", ")
        )));
    }
    Ok(())
}

fn add_expected_staging_file(
    expected: &mut BTreeMap<String, InventoryEntryKind>,
    prefix: &str,
    relative: &str,
) -> Result<()> {
    let components = validate_portable_relative(relative, "allowlisted staging file")?;
    let mut path = prefix.to_owned();
    for (index, component) in components.iter().enumerate() {
        path.push('/');
        path.push_str(component);
        let kind = if index + 1 == components.len() {
            InventoryEntryKind::File
        } else {
            InventoryEntryKind::Directory
        };
        if expected
            .insert(path.clone(), kind)
            .is_some_and(|old| old != kind)
        {
            return Err(CodegenError::new(format!(
                "allowlisted staging path `{path}` is both a file and directory"
            )));
        }
    }
    Ok(())
}

fn audit_captured_staging(
    staging_root: &Path,
    captured_tree: &BTreeMap<String, Vec<u8>>,
) -> Result<AuditReport> {
    let mut workspace = ProposalWorkspace::create(staging_root)?;
    let validation = (|| {
        write_tree_atomically(workspace.repo_root(), captured_tree)?;
        audit(&AuditOptions::new(workspace.repo_root()))
    })();
    let cleanup = workspace.cleanup();
    match (validation, cleanup) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(cleanup_error)) => Err(CodegenError::new(format!(
            "captured staging audit succeeded but cleanup failed: {cleanup_error}"
        ))),
        (Err(error), Err(cleanup_error)) => Err(CodegenError::new(format!(
            "captured staging audit failed: {error}; cleanup also failed: {cleanup_error}"
        ))),
    }
}

fn load_reviewed_artifact_binding(
    packet_root: &Path,
    review_ledger_path: &Path,
) -> Result<ReviewedArtifactBinding> {
    let packet_before = read_tree_stably(packet_root, "reviewed evidence packet")?;
    let verification = verify_evidence(&VerifyEvidenceOptions::new(packet_root))?;
    let packet_tree = read_tree_stably(packet_root, "reviewed evidence packet after verification")?;
    compare_trees(
        "reviewed evidence packet during verification",
        &packet_before,
        &packet_tree,
    )?;

    let manifest_bytes = required_tree_file(
        &packet_tree,
        EVIDENCE_PACKET_MANIFEST,
        "reviewed evidence packet",
    )?;
    let manifest_path = packet_root.join(EVIDENCE_PACKET_MANIFEST);
    let manifest_value = parse_strict(manifest_bytes, &manifest_path)?;
    if canonical_bytes(&manifest_value) != manifest_bytes {
        return Err(CodegenError::new(format!(
            "reviewed packet manifest `{}` is not in exact canonical JSON form",
            manifest_path.display()
        )));
    }
    let manifest: EvidencePacketManifest = serde_json::from_value(manifest_value.into_serde())
        .map_err(|source| {
            CodegenError::with_source("load closed reviewed packet manifest", source)
        })?;
    let recomputed_packet_digest = recompute_packet_digest(&manifest, &packet_tree)?;
    if manifest.packet_digest_sha256 != recomputed_packet_digest {
        return Err(CodegenError::new(format!(
            "reviewed packet manifest digest {} differs from recomputed digest {recomputed_packet_digest}",
            manifest.packet_digest_sha256
        )));
    }
    if verification.review_status != EvidenceReviewStatus::Reviewed
        || manifest.review.status != EvidenceReviewStatus::Reviewed
        || manifest
            .derived_evidence
            .iter()
            .any(|entry| entry.review_status != EvidenceReviewStatus::Reviewed)
    {
        return Err(CodegenError::new(
            "form integration requires a reviewed packet whose complete derived inventory is reviewed",
        ));
    }
    for (label, verified, manifest_value) in [
        (
            "packet_id",
            verification.packet_id.as_str(),
            manifest.packet_id.as_str(),
        ),
        (
            "form_id",
            verification.form_id.as_str(),
            manifest.form_id.as_str(),
        ),
        (
            "packet_digest_sha256",
            verification.packet_digest_sha256.as_str(),
            recomputed_packet_digest.as_str(),
        ),
    ] {
        if verified != manifest_value {
            return Err(CodegenError::new(format!(
                "existing packet verifier {label} `{verified}` differs from snapshotted reviewed packet `{manifest_value}`"
            )));
        }
    }

    let ledger_bytes = read_verified_regular_file(review_ledger_path, "review ledger")?;
    let ledger_value = parse_strict(&ledger_bytes, review_ledger_path)?;
    if canonical_bytes(&ledger_value) != ledger_bytes {
        return Err(CodegenError::new(format!(
            "review ledger `{}` is not in exact canonical JSON form",
            review_ledger_path.display()
        )));
    }
    let ledger: ClosedReviewLedger = serde_json::from_value(ledger_value.into_serde())
        .map_err(|source| CodegenError::with_source("load closed review ledger", source))?;
    let ledger_entry =
        validate_review_ledger_binding(&ledger, &manifest, &recomputed_packet_digest)?;
    let summary_bytes = required_tree_file(
        &packet_tree,
        TRACKED_V1_SUMMARY_PATH,
        "reviewed evidence packet",
    )?;
    validate_review_summary_binding(summary_bytes, &manifest, ledger_entry)?;

    Ok(ReviewedArtifactBinding { manifest })
}

fn recompute_packet_digest(
    manifest: &EvidencePacketManifest,
    packet_tree: &BTreeMap<String, Vec<u8>>,
) -> Result<String> {
    let declared = manifest
        .derived_evidence
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<BTreeSet<_>>();
    let actual = packet_tree
        .keys()
        .filter(|path| path.as_str() != EVIDENCE_PACKET_MANIFEST)
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual != declared {
        return Err(CodegenError::new(
            "reviewed packet filesystem inventory differs from its closed derived-evidence manifest",
        ));
    }

    let mut normalized_manifest = manifest.clone();
    normalized_manifest.packet_digest_sha256.clear();
    let manifest_bytes =
        canonical_serialize(&normalized_manifest, "reviewed packet digest manifest")?;
    let mut entries = Vec::with_capacity(packet_tree.len());
    entries.push((EVIDENCE_PACKET_MANIFEST.to_owned(), manifest_bytes));
    for (path, bytes) in packet_tree {
        if path != EVIDENCE_PACKET_MANIFEST {
            entries.push((path.clone(), bytes.clone()));
        }
    }
    Ok(digest_entries(
        EVIDENCE_PACKET_DIGEST_DOMAIN,
        entries
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    ))
}

fn validate_review_ledger_binding<'a>(
    ledger: &'a ClosedReviewLedger,
    manifest: &EvidencePacketManifest,
    recomputed_packet_digest: &str,
) -> Result<&'a ClosedReviewLedgerEntry> {
    if ledger.format != EVIDENCE_REVIEW_LEDGER_FORMAT
        || ledger.canonicalization != CANONICALIZATION_ID
        || ledger.entries.is_empty()
    {
        return Err(CodegenError::new(format!(
            "review ledger must be non-empty and use `{EVIDENCE_REVIEW_LEDGER_FORMAT}` / `{CANONICALIZATION_ID}`"
        )));
    }

    let mut form_ids = BTreeSet::new();
    let mut packet_ids = BTreeSet::new();
    for entry in &ledger.entries {
        require_one_portable_component(&entry.form_id, "review ledger form_id")?;
        require_one_portable_component(&entry.packet_id, "review ledger packet_id")?;
        require_one_portable_component(
            &entry.official_package_asset_id,
            "review ledger official_package_asset_id",
        )?;
        require_one_portable_component(
            &entry.capture_session_id,
            "review ledger capture_session_id",
        )?;
        if !form_ids.insert(entry.form_id.as_str()) || !packet_ids.insert(entry.packet_id.as_str())
        {
            return Err(CodegenError::new(
                "review ledger contains a duplicate form_id or packet_id",
            ));
        }
        if !is_sha256(&entry.tracked_v1_source_set_sha256)
            || !is_sha256(&entry.source_map_sha256)
            || !is_sha256(&entry.source_verification_sha256)
            || entry
                .expected_packet_digest_sha256
                .as_deref()
                .is_some_and(|digest| !is_sha256(digest))
        {
            return Err(CodegenError::new(
                "review ledger contains a malformed tracked-v1 or expected packet digest",
            ));
        }
    }

    let matching = ledger
        .entries
        .iter()
        .filter(|entry| entry.form_id == manifest.form_id)
        .collect::<Vec<_>>();
    let [entry] = matching.as_slice() else {
        return Err(CodegenError::new(format!(
            "review ledger must contain exactly one entry for packet form_id `{}`, found {}",
            manifest.form_id,
            matching.len()
        )));
    };
    if entry.review.status != EvidenceReviewStatus::Reviewed
        || entry
            .review
            .reviewed_by
            .as_deref()
            .map(str::is_empty)
            .unwrap_or(true)
        || entry
            .review
            .reviewed_at_utc
            .as_deref()
            .map(str::is_empty)
            .unwrap_or(true)
    {
        return Err(CodegenError::new(format!(
            "review ledger entry `{}` is not explicitly reviewed",
            entry.form_id
        )));
    }
    if !matches!(
        &entry.rule_set_source_state,
        RuleSetSourceState::Planned {
            source_set_sha256: ()
        }
    ) {
        return Err(CodegenError::new(format!(
            "review ledger entry `{}` must retain planned/null v2 source state",
            entry.form_id
        )));
    }
    let expected_digest = entry
        .expected_packet_digest_sha256
        .as_deref()
        .ok_or_else(|| {
            CodegenError::new(format!(
                "review ledger entry `{}` has no reviewed expected packet digest",
                entry.form_id
            ))
        })?;
    for (label, ledger_value, packet_value) in [
        (
            "packet_id",
            entry.packet_id.as_str(),
            manifest.packet_id.as_str(),
        ),
        (
            "rule_set_id",
            entry.rule_set_id.as_str(),
            manifest.rule_set_id.as_str(),
        ),
        (
            "tracked_v1_source_set_sha256",
            entry.tracked_v1_source_set_sha256.as_str(),
            manifest.tracked_v1_source_set_sha256.as_str(),
        ),
        (
            "source_map_sha256",
            entry.source_map_sha256.as_str(),
            manifest.source_map_sha256.as_str(),
        ),
        (
            "source_verification_sha256",
            entry.source_verification_sha256.as_str(),
            manifest.source_verification_sha256.as_str(),
        ),
        (
            "expected_packet_digest_sha256",
            expected_digest,
            recomputed_packet_digest,
        ),
        (
            "created_at_utc",
            entry.created_at_utc.as_str(),
            manifest.created_at_utc.as_str(),
        ),
    ] {
        if ledger_value != packet_value {
            return Err(CodegenError::new(format!(
                "review ledger {label} `{ledger_value}` differs from reviewed packet `{packet_value}`"
            )));
        }
    }
    require_same_serialized(
        &entry.capture_provenance,
        &manifest.capture_provenance,
        "capture provenance",
    )?;
    require_same_serialized(&entry.review, &manifest.review, "review metadata")?;
    require_same_serialized(
        &entry.attestations,
        &manifest.attestations,
        "review attestations",
    )?;

    let packet_reviews = manifest
        .derived_evidence
        .iter()
        .map(|derived| (derived.path.as_str(), derived.review_status))
        .collect::<BTreeMap<_, _>>();
    let mut ledger_reviews = BTreeMap::new();
    for review in &entry.derived_reviews {
        validate_portable_relative(&review.path, "review ledger derived-review path")?;
        if review.status != EvidenceReviewStatus::Reviewed
            || ledger_reviews
                .insert(review.path.as_str(), review.status)
                .is_some()
        {
            return Err(CodegenError::new(
                "review ledger derived reviews must be unique and explicitly reviewed",
            ));
        }
    }
    if ledger_reviews != packet_reviews {
        return Err(CodegenError::new(
            "review ledger derived-review inventory differs from the reviewed packet",
        ));
    }
    Ok(entry)
}

fn validate_review_summary_binding(
    bytes: &[u8],
    manifest: &EvidencePacketManifest,
    ledger: &ClosedReviewLedgerEntry,
) -> Result<()> {
    let path = Path::new(TRACKED_V1_SUMMARY_PATH);
    let summary = parse_strict(bytes, path)?;
    let summary = summary.into_serde();
    for (label, actual, expected) in [
        (
            "format",
            required_value_string(&summary, "format", "reviewed packet summary")?,
            EVIDENCE_SUMMARY_FORMAT,
        ),
        (
            "canonicalization",
            required_value_string(&summary, "canonicalization", "reviewed packet summary")?,
            CANONICALIZATION_ID,
        ),
        (
            "form_id",
            required_value_string(&summary, "form_id", "reviewed packet summary")?,
            manifest.form_id.as_str(),
        ),
        (
            "tracked_v1_source_set_sha256",
            required_value_string(
                &summary,
                "tracked_v1_source_set_sha256",
                "reviewed packet summary",
            )?,
            manifest.tracked_v1_source_set_sha256.as_str(),
        ),
    ] {
        if actual != expected {
            return Err(CodegenError::new(format!(
                "reviewed packet summary {label} `{actual}` differs from bound value `{expected}`"
            )));
        }
    }

    let provenance_sha256 = sha256_hex(&canonical_serialize(
        &ledger.capture_provenance,
        "review-ledger capture provenance",
    )?);
    let sessions = summary
        .get("capture_sessions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CodegenError::new("reviewed packet summary is missing capture_sessions array")
        })?;
    let matching_sessions = sessions
        .iter()
        .filter(|session| {
            session.get("capture_session_id").and_then(Value::as_str)
                == Some(ledger.capture_session_id.as_str())
        })
        .collect::<Vec<_>>();
    let [session] = matching_sessions.as_slice() else {
        return Err(CodegenError::new(format!(
            "reviewed packet summary must contain exactly one capture session `{}`, found {}",
            ledger.capture_session_id,
            matching_sessions.len()
        )));
    };
    let source_map_sha256 = required_sha256_value(
        session,
        "source_map_sha256",
        "reviewed packet summary capture session",
    )?;
    let source_verification_sha256 = required_sha256_value(
        session,
        "source_verification_sha256",
        "reviewed packet summary capture session",
    )?;
    if source_map_sha256 != ledger.source_map_sha256
        || source_verification_sha256 != ledger.source_verification_sha256
        || session
            .get("capture_provenance_sha256")
            .and_then(Value::as_str)
            != Some(provenance_sha256.as_str())
        || !session
            .get("upstream_evidence_ids")
            .and_then(Value::as_array)
            .is_some_and(|ids| {
                ids.iter()
                    .any(|id| id.as_str() == Some(manifest.official_package_evidence_id.as_str()))
            })
    {
        return Err(CodegenError::new(
            "reviewed packet summary does not bind the ledger capture session/provenance to the official package evidence",
        ));
    }

    let assets = summary
        .get("upstream_assets")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CodegenError::new("reviewed packet summary is missing upstream_assets array")
        })?;
    let matching_assets = assets
        .iter()
        .filter(|asset| {
            asset.get("asset_id").and_then(Value::as_str)
                == Some(ledger.official_package_asset_id.as_str())
        })
        .collect::<Vec<_>>();
    let [asset] = matching_assets.as_slice() else {
        return Err(CodegenError::new(format!(
            "reviewed packet summary must contain exactly one official package asset `{}`, found {}",
            ledger.official_package_asset_id,
            matching_assets.len()
        )));
    };
    if asset.get("kind").and_then(Value::as_str) != Some("official-package-executable")
        || asset.get("upstream_evidence_id").and_then(Value::as_str)
            != Some(manifest.official_package_evidence_id.as_str())
    {
        return Err(CodegenError::new(
            "reviewed packet summary official package asset is not bound to the manifest evidence identity",
        ));
    }
    Ok(())
}

fn load_handoff(bytes: &[u8], path: &Path) -> Result<PacketBackedHandoff> {
    let parsed = parse_strict(bytes, path)?;
    if canonical_bytes(&parsed) != bytes {
        return Err(CodegenError::new(format!(
            "packet-backed handoff `{}` is not in exact canonical JSON form",
            path.display()
        )));
    }
    serde_json::from_value(parsed.into_serde())
        .map_err(|source| CodegenError::with_source("load closed packet-backed handoff", source))
}

fn validate_handoff(
    handoff: &PacketBackedHandoff,
    snapshot: &crate::audit::AuditedSnapshot,
    manifest: &EvidencePacketManifest,
    tracked_v1_source_set_sha256: &str,
) -> Result<()> {
    validate_handoff_artifact_binding(handoff, manifest, tracked_v1_source_set_sha256)?;
    let identity = &snapshot.document.identity;
    for (label, candidate_value, reviewed_value) in [
        (
            "rule_set_id",
            identity.rule_set_id.as_str(),
            manifest.rule_set_id.as_str(),
        ),
        (
            "form_code",
            identity.form_code.as_str(),
            manifest.form_code.as_str(),
        ),
        (
            "form_revision",
            identity.form_revision.as_str(),
            manifest.form_revision.as_str(),
        ),
        (
            "official_package_version",
            identity.official_package_version.as_str(),
            manifest.official_package_version.as_str(),
        ),
    ] {
        if candidate_value != reviewed_value {
            return Err(CodegenError::new(format!(
                "audited candidate {label} `{candidate_value}` differs from independently verified packet `{reviewed_value}`"
            )));
        }
    }
    Ok(())
}

fn validate_handoff_artifact_binding(
    handoff: &PacketBackedHandoff,
    manifest: &EvidencePacketManifest,
    tracked_v1_source_set_sha256: &str,
) -> Result<()> {
    if handoff.format != PACKET_BACKED_HANDOFF_FORMAT
        || handoff.canonicalization != CANONICALIZATION_ID
    {
        return Err(CodegenError::new(
            "staging handoff does not have the packet-backed form handoff identity",
        ));
    }
    if handoff.review_status != "skeleton"
        || handoff.canonical_integration_performed
        || handoff.proves_executable_semantics
    {
        return Err(CodegenError::new(
            "staging handoff claims review, prior canonical integration, or executable proof",
        ));
    }
    if handoff.packet.review_status != "reviewed"
        || handoff.packet.record_census_path != "derived/tracked-v1-summary.json"
    {
        return Err(CodegenError::new(
            "staging handoff does not bind an exact reviewed packet and record census",
        ));
    }
    for (label, handoff_value, reviewed_value) in [
        (
            "packet_id",
            handoff.packet.packet_id.as_str(),
            manifest.packet_id.as_str(),
        ),
        (
            "packet_digest_sha256",
            handoff.packet.packet_digest_sha256.as_str(),
            manifest.packet_digest_sha256.as_str(),
        ),
        (
            "form_id",
            handoff.identity.form_id.as_str(),
            manifest.form_id.as_str(),
        ),
        (
            "tracked_v1_source_set_sha256",
            handoff.identity.tracked_v1_source_set_sha256.as_str(),
            tracked_v1_source_set_sha256,
        ),
    ] {
        if handoff_value != reviewed_value {
            return Err(CodegenError::new(format!(
                "staging handoff {label} `{handoff_value}` differs from independently verified artifact `{reviewed_value}`"
            )));
        }
    }
    if handoff.identity.source_set_sha256.is_some() || !is_sha256(tracked_v1_source_set_sha256) {
        return Err(CodegenError::new(
            "packet-backed handoff must retain planned/null v2 state and an exact tracked-v1 digest",
        ));
    }
    for (label, expected, actual) in [
        (
            "rule_set_id",
            manifest.rule_set_id.as_str(),
            handoff.identity.rule_set_id.as_str(),
        ),
        (
            "form_code",
            manifest.form_code.as_str(),
            handoff.identity.form_code.as_str(),
        ),
        (
            "form_revision",
            manifest.form_revision.as_str(),
            handoff.identity.form_revision.as_str(),
        ),
        (
            "official_package_version",
            manifest.official_package_version.as_str(),
            handoff.identity.official_package_version.as_str(),
        ),
    ] {
        if expected != actual {
            return Err(CodegenError::new(format!(
                "staging handoff {label} `{actual}` differs from independently verified packet `{expected}`"
            )));
        }
    }
    if handoff.identity.form_id.is_empty()
        || handoff.blocking_gaps.is_empty()
        || handoff.legacy_record_census.is_null()
        || handoff.serialization_occurrences.is_null()
    {
        return Err(CodegenError::new(
            "staging handoff is missing its form identity or fail-closed census/gap payload",
        ));
    }
    Ok(())
}

fn validate_closed_snapshot_identity(
    snapshot: &ClosedIndexSnapshot,
    manifest: &EvidencePacketManifest,
) -> Result<()> {
    for (label, candidate_value, packet_value) in [
        (
            "rule_set_id",
            snapshot.rule_set_id.as_str(),
            manifest.rule_set_id.as_str(),
        ),
        (
            "form_code",
            snapshot.form_code.as_str(),
            manifest.form_code.as_str(),
        ),
        (
            "form_revision",
            snapshot.form_revision.as_str(),
            manifest.form_revision.as_str(),
        ),
        (
            "official_package_version",
            snapshot.official_package_version.as_str(),
            manifest.official_package_version.as_str(),
        ),
    ] {
        if candidate_value != packet_value {
            return Err(CodegenError::new(format!(
                "staged v2 index {label} `{candidate_value}` differs from independently verified packet `{packet_value}`"
            )));
        }
    }
    let source_set_sha256 = snapshot.source_set_sha256.as_deref().ok_or_else(|| {
        CodegenError::new(format!(
            "staged candidate index `{}` must carry its computed source_set_sha256; planned/null state belongs only to the packet handoff",
            snapshot.rule_set_id
        ))
    })?;
    if !is_sha256(source_set_sha256) {
        return Err(CodegenError::new(format!(
            "staged candidate index `{}` has malformed source_set_sha256 `{source_set_sha256}`",
            snapshot.rule_set_id
        )));
    }
    Ok(())
}

fn validate_v1_manifest_identity(
    form_root: &Path,
    form_tree: &BTreeMap<String, Vec<u8>>,
    packet: &EvidencePacketManifest,
) -> Result<()> {
    let bytes = required_tree_file(form_tree, "manifest.json", "canonical v1 form tree")?;
    let value = parse_strict(bytes, &form_root.join("manifest.json"))?.into_serde();
    for (label, key, packet_value) in [
        ("form_id", "form_id", packet.form_id.as_str()),
        ("form_code", "form_code", packet.form_code.as_str()),
        ("form_revision", "revision", packet.form_revision.as_str()),
        (
            "official_package_version",
            "package_version",
            packet.official_package_version.as_str(),
        ),
    ] {
        let canonical_value = required_value_string(&value, key, "canonical v1 manifest")?;
        if canonical_value != packet_value {
            return Err(CodegenError::new(format!(
                "canonical v1 manifest {label} `{canonical_value}` differs from reviewed packet `{packet_value}`"
            )));
        }
    }
    Ok(())
}

fn tracked_v1_source_set_digest(
    form_root: &Path,
    form_tree: &BTreeMap<String, Vec<u8>>,
) -> Result<String> {
    let mut sources = Vec::new();
    for (path, bytes) in form_tree {
        let name = path.rsplit('/').next().unwrap_or(path.as_str());
        if matches!(name, "README.md" | "HANDOFF.md") || name.starts_with("v2-") {
            continue;
        }
        let canonical = if path.ends_with(".json") {
            canonical_bytes(&parse_strict(
                bytes,
                &form_root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR)),
            )?)
        } else if path.ends_with(".md") {
            canonical_tracked_text(bytes, path)?
        } else {
            return Err(CodegenError::new(format!(
                "tracked v1 source `{path}` has unsupported non-text extension"
            )));
        };
        sources.push((path.clone(), canonical));
    }
    let source_paths = sources
        .iter()
        .map(|(path, _)| path.as_str())
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
        if !source_paths.contains(required) {
            return Err(CodegenError::new(format!(
                "tracked v1 source set is missing required `{required}`"
            )));
        }
    }
    Ok(digest_entries(
        TRACKED_V1_SOURCE_SET_DOMAIN,
        sources
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    ))
}

fn canonical_tracked_text(bytes: &[u8], path: &str) -> Result<Vec<u8>> {
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

fn require_allowed_review_state(snapshot: &ClosedIndexSnapshot) -> Result<()> {
    match snapshot.review_status {
        ClosedReviewStatus::Candidate => Ok(()),
        ClosedReviewStatus::Skeleton => Err(CodegenError::new(format!(
            "staged rule set `{}` is still skeleton; canonical integration requires a candidate",
            snapshot.rule_set_id
        ))),
        ClosedReviewStatus::Reviewed => Err(CodegenError::new(format!(
            "staged rule set `{}` is reviewed; promotion cannot occur through form integration",
            snapshot.rule_set_id
        ))),
    }
}

fn require_single_staged_directory(
    staged: &BTreeMap<String, Vec<u8>>,
    rule_set_id: &str,
) -> Result<()> {
    let rule_set_path = format!("{rule_set_id}/rule-set.json");
    if !staged.contains_key(INDEX_PATH) || !staged.contains_key(&rule_set_path) {
        return Err(CodegenError::new(format!(
            "staged v2 tree must contain only an index and `{rule_set_path}` snapshot directory"
        )));
    }
    let prefix = format!("{rule_set_id}/");
    for path in staged.keys() {
        if path != INDEX_PATH && !path.starts_with(&prefix) {
            return Err(CodegenError::new(format!(
                "staged v2 tree contains path `{path}` outside the single `{rule_set_id}` directory"
            )));
        }
    }
    Ok(())
}

fn reject_existing_canonical_target(source_root: &Path, rule_set_id: &str) -> Result<()> {
    let mut entries = fs::read_dir(source_root)
        .map_err(|source| CodegenError::io("read canonical v2 source root", source_root, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| {
            CodegenError::io("read canonical v2 source entry", source_root, source)
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry
            .file_name()
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "canonical v2 source entry `{}` is not valid UTF-8",
                    entry.path().display()
                ))
            })?;
        if name.to_lowercase() == rule_set_id.to_lowercase() {
            return Err(CodegenError::new(format!(
                "canonical v2 source target `{}` already exists; refusing overwrite",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn require_add_only_delta(
    current: &BTreeMap<String, Vec<u8>>,
    staged: &BTreeMap<String, Vec<u8>>,
    proposed: &BTreeMap<String, Vec<u8>>,
    rule_set_id: &str,
) -> Result<()> {
    for (path, bytes) in current {
        if path == INDEX_PATH {
            continue;
        }
        if proposed.get(path) != Some(bytes) {
            return Err(CodegenError::new(format!(
                "proposed integration changes existing canonical file `{path}`"
            )));
        }
    }
    let prefix = format!("{rule_set_id}/");
    let expected_added = staged
        .iter()
        .filter(|(path, _)| path.starts_with(&prefix))
        .collect::<BTreeMap<_, _>>();
    let actual_added = proposed
        .iter()
        .filter(|(path, _)| !current.contains_key(path.as_str()) && path.starts_with(&prefix))
        .collect::<BTreeMap<_, _>>();
    if expected_added != actual_added {
        return Err(CodegenError::new(
            "proposed integration is not the exact staged snapshot-directory addition",
        ));
    }
    let allowed_count = current.len() + expected_added.len();
    if proposed.len() != allowed_count {
        return Err(CodegenError::new(format!(
            "proposed v2 tree has {} files, expected exactly {allowed_count}",
            proposed.len()
        )));
    }
    Ok(())
}

fn require_protected_tree(tree: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let prefix = format!("{PROTECTED_2550Q_RULE_SET_ID}/");
    if tree.keys().any(|path| path.starts_with(&prefix)) {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "canonical v2 tree is missing protected directory `{PROTECTED_2550Q_RULE_SET_ID}`"
        )))
    }
}

fn require_protected_tree_unchanged(
    current: &BTreeMap<String, Vec<u8>>,
    proposed: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let prefix = format!("{PROTECTED_2550Q_RULE_SET_ID}/");
    let current_protected = extract_subtree(current, &prefix);
    let proposed_protected = extract_subtree(proposed, &prefix);
    compare_trees(
        "protected 2550Q snapshot",
        &current_protected,
        &proposed_protected,
    )
}

fn require_protected_audit(report: &AuditReport, expected: &ClosedIndexSnapshot) -> Result<()> {
    let protected = report.require_rule_set(PROTECTED_2550Q_RULE_SET_ID)?;
    if protected.rule_set_id() != expected.rule_set_id
        || protected.form_code() != expected.form_code
        || protected.form_revision() != expected.form_revision
        || protected.official_package_version() != expected.official_package_version
        || protected.source_set_sha256() != expected.source_set_sha256.as_deref()
        || protected.review_status() != expected.review_status.as_str()
    {
        return Err(CodegenError::new(
            "aggregate audit changed the protected 2550Q exact identity or review state",
        ));
    }
    Ok(())
}

fn parse_index(bytes: &[u8], path: &Path) -> Result<ClosedIndex> {
    let parsed = parse_strict(bytes, path)?;
    serde_json::from_value(parsed.into_serde())
        .map_err(|source| CodegenError::with_source("load closed v2 integration index", source))
}

fn render_index(index: &ClosedIndex) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(index)
        .map_err(|source| CodegenError::with_source("serialize proposed v2 index", source))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_rule_set_id(rule_set_id: &str) -> Result<()> {
    let components = validate_portable_relative(rule_set_id, "rule_set_id")?;
    if components.len() != 1 || rule_set_id == INDEX_PATH {
        return Err(CodegenError::new(format!(
            "rule_set_id `{rule_set_id}` must be one portable path component"
        )));
    }
    Ok(())
}

fn canonical_external_directory(path: &Path, repo_root: &Path, label: &str) -> Result<PathBuf> {
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be lexically normalized",
            path.display()
        )));
    }
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if is_symlink_or_reparse_point(&metadata) => {
                return Err(CodegenError::new(format!(
                    "{label} `{}` traverses symlink or reparse point `{}`",
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
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| CodegenError::io(&format!("inspect {label}"), path, source))?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be a real directory",
            path.display()
        )));
    }
    let resolved = fs::canonicalize(path)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label}"), path, source))?;
    if is_same_or_below(repo_root, &resolved) || is_same_or_below(&resolved, repo_root) {
        return Err(CodegenError::new(format!(
            "{label} `{}` overlaps canonical repository `{}`",
            resolved.display(),
            repo_root.display()
        )));
    }
    Ok(resolved)
}

fn canonical_external_file(path: &Path, repo_root: &Path, label: &str) -> Result<PathBuf> {
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be lexically normalized",
            path.display()
        )));
    }
    for ancestor in path.ancestors() {
        let metadata = match fs::symlink_metadata(ancestor) {
            Ok(metadata) => metadata,
            Err(source)
                if source.kind() == std::io::ErrorKind::NotFound
                    && ancestor.as_os_str().is_empty() =>
            {
                continue;
            }
            Err(source) => {
                return Err(CodegenError::io(
                    &format!("inspect {label} ancestor"),
                    ancestor,
                    source,
                ));
            }
        };
        if is_symlink_or_reparse_point(&metadata) {
            return Err(CodegenError::new(format!(
                "{label} `{}` traverses symlink or reparse point `{}`",
                path.display(),
                ancestor.display()
            )));
        }
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| CodegenError::io(&format!("inspect {label}"), path, source))?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_file() {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be a real file",
            path.display()
        )));
    }
    let resolved = fs::canonicalize(path)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label}"), path, source))?;
    if is_same_or_below(repo_root, &resolved) {
        return Err(CodegenError::new(format!(
            "{label} `{}` is inside canonical repository `{}`",
            resolved.display(),
            repo_root.display()
        )));
    }
    Ok(resolved)
}

fn require_disjoint_directories(
    left: &Path,
    left_label: &str,
    right: &Path,
    right_label: &str,
) -> Result<()> {
    if is_same_or_below(left, right) || is_same_or_below(right, left) {
        return Err(CodegenError::new(format!(
            "{left_label} `{}` and {right_label} `{}` must not overlap",
            left.display(),
            right.display()
        )));
    }
    Ok(())
}

fn require_file_outside_directory(
    file: &Path,
    file_label: &str,
    directory: &Path,
    directory_label: &str,
) -> Result<()> {
    if is_same_or_below(directory, file) {
        return Err(CodegenError::new(format!(
            "{file_label} `{}` must not be inside {directory_label} `{}`",
            file.display(),
            directory.display()
        )));
    }
    Ok(())
}

fn read_tree_stably(directory: &Path, label: &str) -> Result<BTreeMap<String, Vec<u8>>> {
    let before_metadata = fs::symlink_metadata(directory)
        .map_err(|source| CodegenError::io(&format!("inspect {label}"), directory, source))?;
    if is_symlink_or_reparse_point(&before_metadata) || !before_metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be a real directory",
            directory.display()
        )));
    }
    let before = Handle::from_path(directory)
        .map_err(|source| CodegenError::io(&format!("open {label} identity"), directory, source))?;
    let tree = read_tree(directory)?;
    let after_metadata = fs::symlink_metadata(directory).map_err(|source| {
        CodegenError::io(&format!("reinspect {label} after read"), directory, source)
    })?;
    if is_symlink_or_reparse_point(&after_metadata) || !after_metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "{label} `{}` was replaced by a non-directory or link during read",
            directory.display()
        )));
    }
    let after = Handle::from_path(directory).map_err(|source| {
        CodegenError::io(
            &format!("reopen {label} identity after read"),
            directory,
            source,
        )
    })?;
    if before != after {
        return Err(CodegenError::new(format!(
            "{label} `{}` was replaced during read",
            directory.display()
        )));
    }
    Ok(tree)
}

fn read_verified_regular_file(path: &Path, label: &str) -> Result<Vec<u8>> {
    let mut verified = open_verified_regular_file(path, label, |_| Ok(()))?;
    let mut bytes = Vec::new();
    verified
        .file_mut()
        .read_to_end(&mut bytes)
        .map_err(|source| CodegenError::io(&format!("read {label}"), path, source))?;
    Ok(bytes)
}

fn require_one_portable_component(value: &str, label: &str) -> Result<()> {
    let components = validate_portable_relative(value, label)?;
    if components.len() != 1 {
        return Err(CodegenError::new(format!(
            "{label} `{value}` must be one portable path component"
        )));
    }
    Ok(())
}

fn required_value_string<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| CodegenError::new(format!("{label} is missing required string `{key}`")))
}

fn required_sha256_value<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a str> {
    let digest = required_value_string(value, key, label)?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CodegenError::new(format!(
            "{label} `{key}` must be exactly 64 lowercase hexadecimal characters"
        )));
    }
    Ok(digest)
}

fn canonical_serialize(value: &impl Serialize, label: &str) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value)
        .map_err(|source| CodegenError::with_source(format!("serialize {label}"), source))?;
    Ok(canonical_bytes(&parse_strict(&bytes, Path::new(label))?))
}

fn require_same_serialized(
    left: &impl Serialize,
    right: &impl Serialize,
    label: &str,
) -> Result<()> {
    if canonical_serialize(left, label)? != canonical_serialize(right, label)? {
        return Err(CodegenError::new(format!(
            "review ledger {label} differs from the reviewed packet"
        )));
    }
    Ok(())
}

fn fresh_unpredictable_token(sequence: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let first_state = RandomState::new();
    let mut first = first_state.build_hasher();
    first.write_u64(std::process::id().into());
    first.write_u64(sequence);
    first.write_u128(now);
    first.write_usize((&first_state as *const RandomState) as usize);
    let first_value = first.finish();
    let second_state = RandomState::new();
    let mut second = second_state.build_hasher();
    second.write_u64(first_value);
    second.write_u64(sequence.rotate_left(29));
    second.write_u128(now.rotate_left(47));
    second.write_usize((&second_state as *const RandomState) as usize);
    format!("{first_value:016x}{:016x}", second.finish())
}

fn require_json_only_tree(label: &str, tree: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    if tree.is_empty() {
        return Err(CodegenError::new(format!("{label} must not be empty")));
    }
    for path in tree.keys() {
        if !path.ends_with(".json") {
            return Err(CodegenError::new(format!(
                "{label} contains non-JSON file `{path}`"
            )));
        }
        validate_portable_relative(path, label)?;
    }
    Ok(())
}

fn reject_case_fold_collisions<'a>(
    label: &str,
    paths: impl IntoIterator<Item = &'a String>,
) -> Result<()> {
    let mut seen = BTreeMap::new();
    for path in paths {
        let folded = path.to_lowercase();
        if let Some(previous) = seen.insert(folded, path.as_str()) {
            return Err(CodegenError::new(format!(
                "{label} contains cross-platform path collision `{previous}` / `{path}`"
            )));
        }
    }
    Ok(())
}

fn required_tree_file<'a>(
    tree: &'a BTreeMap<String, Vec<u8>>,
    path: &str,
    label: &str,
) -> Result<&'a [u8]> {
    tree.get(path)
        .map(Vec::as_slice)
        .ok_or_else(|| CodegenError::new(format!("{label} is missing `{path}`")))
}

fn compare_trees(
    label: &str,
    expected: &BTreeMap<String, Vec<u8>>,
    actual: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    if expected == actual {
        return Ok(());
    }
    let expected_paths = expected.keys().cloned().collect::<BTreeSet<_>>();
    let actual_paths = actual.keys().cloned().collect::<BTreeSet<_>>();
    let missing = expected_paths
        .difference(&actual_paths)
        .cloned()
        .collect::<Vec<_>>();
    let extra = actual_paths
        .difference(&expected_paths)
        .cloned()
        .collect::<Vec<_>>();
    let changed = expected_paths
        .intersection(&actual_paths)
        .filter(|path| expected.get(path.as_str()) != actual.get(path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    Err(CodegenError::new(format!(
        "{label} differs: missing=[{}] extra=[{}] changed=[{}]",
        missing.join(", "),
        extra.join(", "),
        changed.join(", ")
    )))
}

fn extract_subtree(tree: &BTreeMap<String, Vec<u8>>, prefix: &str) -> BTreeMap<String, Vec<u8>> {
    tree.iter()
        .filter_map(|(path, bytes)| {
            path.strip_prefix(prefix)
                .map(|relative| (relative.to_owned(), bytes.clone()))
        })
        .collect()
}

fn without_subtree(tree: &BTreeMap<String, Vec<u8>>, prefix: &str) -> BTreeMap<String, Vec<u8>> {
    let root = prefix.trim_end_matches('/');
    tree.iter()
        .filter(|(path, _)| path.as_str() != root && !path.starts_with(prefix))
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect()
}

fn tree_digest(tree: &BTreeMap<String, Vec<u8>>) -> String {
    digest_entries(
        FORM_INTEGRATION_TREE_DIGEST_DOMAIN,
        tree.iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    )
}

fn file_manifest(tree: &BTreeMap<String, Vec<u8>>) -> Vec<FormIntegrationFile> {
    tree.iter()
        .map(|(path, bytes)| FormIntegrationFile {
            path: path.clone(),
            size_bytes: bytes.len() as u64,
            sha256: sha256_hex(bytes),
        })
        .collect()
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

struct ProposalWorkspace {
    container: PathBuf,
    repo_root: PathBuf,
    owner_marker: Vec<u8>,
    identity: Handle,
    cleaned: bool,
}

impl ProposalWorkspace {
    fn create(staging_root: &Path) -> Result<Self> {
        let parent = fs::canonicalize(std::env::temp_dir()).map_err(|source| {
            CodegenError::io(
                "canonicalize private proposal temporary root",
                &std::env::temp_dir(),
                source,
            )
        })?;
        let parent_metadata = fs::symlink_metadata(&parent).map_err(|source| {
            CodegenError::io("inspect private proposal temporary root", &parent, source)
        })?;
        if is_symlink_or_reparse_point(&parent_metadata) || !parent_metadata.is_dir() {
            return Err(CodegenError::new(format!(
                "private proposal temporary root `{}` must be a real directory",
                parent.display()
            )));
        }
        for _ in 0..1_000 {
            let sequence = PROPOSAL_COUNTER.fetch_add(1, Ordering::Relaxed);
            let token = fresh_unpredictable_token(sequence);
            let container = parent.join(format!("{PROPOSAL_PREFIX}-{token}"));
            match fs::create_dir(&container) {
                Ok(()) => {
                    let identity = Handle::from_path(&container).map_err(|source| {
                        CodegenError::io(
                            "identify integration proposal container",
                            &container,
                            source,
                        )
                    })?;
                    let owner_marker = format!(
                        "{PROPOSAL_PREFIX}:owner:{}",
                        fresh_unpredictable_token(sequence.wrapping_add(1))
                    )
                    .into_bytes();
                    let owner_path = container.join(PROPOSAL_OWNER_FILE);
                    let marker_result = (|| {
                        let mut options = OpenOptions::new();
                        options.create_new(true).write(true);
                        let mut file = options.open(&owner_path).map_err(|source| {
                            CodegenError::io(
                                "create integration proposal owner marker",
                                &owner_path,
                                source,
                            )
                        })?;
                        file.write_all(&owner_marker).map_err(|source| {
                            CodegenError::io(
                                "write integration proposal owner marker",
                                &owner_path,
                                source,
                            )
                        })?;
                        file.sync_all().map_err(|source| {
                            CodegenError::io(
                                "sync integration proposal owner marker",
                                &owner_path,
                                source,
                            )
                        })
                    })();
                    if let Err(error) = marker_result {
                        return Err(CodegenError::new(format!(
                            "{error}; incomplete proposal `{}` was left in place to avoid deleting a concurrently substituted path",
                            container.display()
                        )));
                    }
                    return Ok(Self {
                        repo_root: container.join("repo"),
                        container,
                        owner_marker,
                        identity,
                        cleaned: false,
                    });
                }
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(CodegenError::io(
                        "create external proposal container",
                        &container,
                        source,
                    ));
                }
            }
        }
        Err(CodegenError::new(format!(
            "could not allocate a private proposal workspace while validating `{}`",
            staging_root.display()
        )))
    }

    fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    fn cleanup(&mut self) -> Result<()> {
        remove_proposal_container(&self.container, &self.owner_marker, &self.identity)?;
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for ProposalWorkspace {
    fn drop(&mut self) {
        if !self.cleaned {
            let _ = remove_proposal_container(&self.container, &self.owner_marker, &self.identity);
        }
    }
}

fn remove_proposal_container(
    path: &Path,
    owner_marker: &[u8],
    expected_identity: &Handle,
) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let Some(token) = name.strip_prefix(&format!("{PROPOSAL_PREFIX}-")) else {
        return Err(CodegenError::new(format!(
            "refusing to remove non-integration proposal path `{}`",
            path.display()
        )));
    };
    if token.len() != 32 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CodegenError::new(format!(
            "refusing to remove malformed integration proposal path `{}`",
            path.display()
        )));
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(CodegenError::io(
                "inspect integration proposal container",
                path,
                source,
            ));
        }
    };
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "refusing to remove non-directory integration proposal `{}`",
            path.display()
        )));
    }
    let current_identity = Handle::from_path(path).map_err(|source| {
        CodegenError::io("reidentify integration proposal container", path, source)
    })?;
    if &current_identity != expected_identity {
        return Err(CodegenError::new(format!(
            "refusing to remove replaced integration proposal container `{}`",
            path.display()
        )));
    }
    let owner_path = path.join(PROPOSAL_OWNER_FILE);
    if read_verified_regular_file(&owner_path, "integration proposal owner marker")? != owner_marker
    {
        return Err(CodegenError::new(format!(
            "refusing to remove integration proposal whose owner marker changed at `{}`",
            owner_path.display()
        )));
    }
    // `remove_dir_all` is the cross-platform primitive whose contract is to
    // remove symlinks themselves rather than follow them. A pre-walk followed
    // by path-recursive deletion is specifically avoided: that pattern admits
    // an attacker-replaced-entry TOCTOU.
    fs::remove_dir_all(path).map_err(|source| {
        CodegenError::io("remove owned integration proposal container", path, source)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::Ordering;

    use serde_json::json;

    use super::{
        ClosedBranchState, ClosedDerivedReview, ClosedIndex, ClosedIndexSnapshot,
        ClosedProfileStates, ClosedReviewLedger, ClosedReviewLedgerEntry, ClosedReviewStatus,
        EVIDENCE_SUMMARY_FORMAT, HANDOFF_PATH, HandoffIdentity, HandoffPacket, INDEX_PATH,
        PACKET_BACKED_HANDOFF_FORMAT, PROPOSAL_COUNTER, PROPOSAL_OWNER_FILE,
        PROTECTED_2550Q_RULE_SET_ID, PacketBackedHandoff, ProposalWorkspace, canonical_serialize,
        capture_staging_workspace, compare_trees, parse_index, read_verified_regular_file,
        reject_case_fold_collisions, reject_executable_filing_safe_json, render_index,
        require_add_only_delta, require_allowed_review_state, require_protected_tree_unchanged,
        tree_digest, validate_closed_snapshot_identity, validate_external_proposal,
        validate_handoff_artifact_binding, validate_review_ledger_binding,
        validate_review_summary_binding,
    };
    use crate::evidence::{
        EVIDENCE_PACKET_FORMAT, EvidenceCaptureOperatingSystem, EvidenceCaptureProvenance,
        EvidencePacketManifest, EvidenceReview, EvidenceReviewStatus, RuleSetSourceState,
    };
    use crate::files::{read_tree, write_tree_atomically};
    use crate::hash::sha256_hex;
    use crate::json::{CANONICALIZATION_ID, parse_strict};

    #[test]
    fn integration_external_file_read_rejects_hard_link_aliases() {
        let sequence = PROPOSAL_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "bir-rules-codegen-integration-hard-link-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create hard-link test root");
        let source = root.join("source.json");
        let alias = root.join("review-ledger.json");
        fs::write(&source, b"reviewed").expect("write source");
        fs::hard_link(&source, &alias).expect("create hard-link alias");

        let error = read_verified_regular_file(&alias, "test review ledger")
            .expect_err("hard-linked integration input must fail closed");
        assert!(error.to_string().contains("hard links"));

        fs::remove_dir_all(root).expect("remove hard-link test root");
    }

    fn snapshot(rule_set_id: &str, review_status: ClosedReviewStatus) -> ClosedIndexSnapshot {
        ClosedIndexSnapshot {
            rule_set_id: rule_set_id.to_owned(),
            form_code: rule_set_id.to_ascii_uppercase(),
            form_revision: "2026-01-01".to_owned(),
            official_package_version: "7.9.6.0".to_owned(),
            source_set_sha256: None,
            path: format!("{rule_set_id}/rule-set.json"),
            review_status,
            profile_states: ClosedProfileStates {
                official: ClosedBranchState::Unresolved,
                filing_safe: ClosedBranchState::Unresolved,
            },
        }
    }

    fn reviewed_manifest() -> EvidencePacketManifest {
        EvidencePacketManifest {
            format: EVIDENCE_PACKET_FORMAT.to_owned(),
            canonicalization: CANONICALIZATION_ID.to_owned(),
            packet_id: "form-a-packet".to_owned(),
            form_id: "form-a".to_owned(),
            rule_set_id: "form-a-p7.9.6.0".to_owned(),
            tracked_v1_source_set_sha256: "1".repeat(64),
            rule_set_source_state: RuleSetSourceState::Planned {
                source_set_sha256: (),
            },
            form_code: "FORM-A".to_owned(),
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
                windows_version: "Windows 11".to_owned(),
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
            derived_evidence: Vec::new(),
            packet_digest_sha256: "2".repeat(64),
        }
    }

    fn reviewed_ledger_entry(manifest: &EvidencePacketManifest) -> ClosedReviewLedgerEntry {
        ClosedReviewLedgerEntry {
            form_id: manifest.form_id.clone(),
            packet_id: manifest.packet_id.clone(),
            rule_set_id: manifest.rule_set_id.clone(),
            tracked_v1_source_set_sha256: manifest.tracked_v1_source_set_sha256.clone(),
            rule_set_source_state: RuleSetSourceState::Planned {
                source_set_sha256: (),
            },
            official_package_asset_id: "official-package".to_owned(),
            capture_session_id: "capture-session".to_owned(),
            source_map_sha256: manifest.source_map_sha256.clone(),
            source_verification_sha256: manifest.source_verification_sha256.clone(),
            capture_provenance: manifest.capture_provenance.clone(),
            created_at_utc: manifest.created_at_utc.clone(),
            review: manifest.review.clone(),
            attestations: manifest.attestations.clone(),
            derived_reviews: Vec::<ClosedDerivedReview>::new(),
            source_excerpts: Vec::new(),
            capture_gaps: Vec::new(),
            expected_packet_digest_sha256: Some(manifest.packet_digest_sha256.clone()),
        }
    }

    fn skeleton_handoff(manifest: &EvidencePacketManifest) -> PacketBackedHandoff {
        PacketBackedHandoff {
            format: PACKET_BACKED_HANDOFF_FORMAT.to_owned(),
            canonicalization: CANONICALIZATION_ID.to_owned(),
            packet: HandoffPacket {
                packet_id: manifest.packet_id.clone(),
                packet_digest_sha256: manifest.packet_digest_sha256.clone(),
                review_status: "reviewed".to_owned(),
                record_census_path: "derived/tracked-v1-summary.json".to_owned(),
            },
            identity: HandoffIdentity {
                form_id: manifest.form_id.clone(),
                form_code: manifest.form_code.clone(),
                form_revision: manifest.form_revision.clone(),
                official_package_version: manifest.official_package_version.clone(),
                rule_set_id: manifest.rule_set_id.clone(),
                source_set_sha256: None,
                tracked_v1_source_set_sha256: manifest.tracked_v1_source_set_sha256.clone(),
            },
            review_status: "skeleton".to_owned(),
            canonical_integration_performed: false,
            proves_executable_semantics: false,
            legacy_record_census: json!({"validations": 1}),
            serialization_occurrences: json!({"state": "unresolved"}),
            blocking_gaps: vec!["official-profile-unresolved".to_owned()],
        }
    }

    fn staging_layout_fixture(
        root: &Path,
    ) -> (BTreeMap<String, Vec<u8>>, BTreeMap<String, Vec<u8>>) {
        let form_tree = BTreeMap::from([("manifest.json".to_owned(), b"{}\n".to_vec())]);
        let schema_tree = BTreeMap::from([("rule-set.schema.json".to_owned(), b"{}\n".to_vec())]);
        for (relative, bytes) in [
            ("HANDOFF.json", b"{\"epoch\":1}\n".as_slice()),
            ("HANDOFF.md", b"# handoff\n".as_slice()),
            ("rules/forms/form-a/manifest.json", b"{}\n".as_slice()),
            ("rules/schema/v2/rule-set.schema.json", b"{}\n".as_slice()),
            ("rules/ir/v2/index.json", b"{}\n".as_slice()),
            (
                "rules/ir/v2/form-a-p7.9.6.0/rule-set.json",
                b"{}\n".as_slice(),
            ),
        ] {
            let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
            fs::create_dir_all(path.parent().expect("staging fixture parent"))
                .expect("create staging fixture directory");
            fs::write(path, bytes).expect("write staging fixture");
        }
        (form_tree, schema_tree)
    }

    #[test]
    fn add_only_merge_preserves_every_existing_snapshot_byte() {
        let protected_path = format!("{PROTECTED_2550Q_RULE_SET_ID}/rule-set.json");
        let staged_path = "1701q-v2018-p7.9.6.0/rule-set.json".to_owned();
        let current = BTreeMap::from([
            (INDEX_PATH.to_owned(), b"old-index".to_vec()),
            (protected_path, b"protected".to_vec()),
        ]);
        let staged = BTreeMap::from([
            (INDEX_PATH.to_owned(), b"staged-index".to_vec()),
            (staged_path.clone(), b"staged".to_vec()),
        ]);
        let proposed = BTreeMap::from([
            (INDEX_PATH.to_owned(), b"new-index".to_vec()),
            (
                format!("{PROTECTED_2550Q_RULE_SET_ID}/rule-set.json"),
                b"protected".to_vec(),
            ),
            (staged_path, b"staged".to_vec()),
        ]);

        require_add_only_delta(&current, &staged, &proposed, "1701q-v2018-p7.9.6.0")
            .expect("exact addition");
        require_protected_tree_unchanged(&current, &proposed).expect("2550Q unchanged");
        assert_ne!(tree_digest(&current), tree_digest(&proposed));
    }

    #[test]
    fn overwrite_and_case_fold_collision_fail_closed() {
        let current = BTreeMap::from([
            (INDEX_PATH.to_owned(), b"old-index".to_vec()),
            (
                format!("{PROTECTED_2550Q_RULE_SET_ID}/rule-set.json"),
                b"protected".to_vec(),
            ),
        ]);
        let staged = BTreeMap::from([
            (INDEX_PATH.to_owned(), b"staged-index".to_vec()),
            (
                format!("{PROTECTED_2550Q_RULE_SET_ID}/rule-set.json"),
                b"replacement".to_vec(),
            ),
        ]);
        let proposed = BTreeMap::from([
            (INDEX_PATH.to_owned(), b"new-index".to_vec()),
            (
                format!("{PROTECTED_2550Q_RULE_SET_ID}/rule-set.json"),
                b"replacement".to_vec(),
            ),
        ]);
        assert!(
            require_add_only_delta(&current, &staged, &proposed, PROTECTED_2550Q_RULE_SET_ID)
                .is_err()
        );
        assert!(require_protected_tree_unchanged(&current, &proposed).is_err());

        let paths = [
            "form/fixture.json".to_owned(),
            "FORM/FIXTURE.json".to_owned(),
        ];
        assert!(reject_case_fold_collisions("test tree", paths.iter()).is_err());
    }

    #[test]
    fn only_candidate_input_is_an_integration_state() {
        let reviewed = snapshot("1701q-v2018-p7.9.6.0", ClosedReviewStatus::Reviewed);
        assert!(require_allowed_review_state(&reviewed).is_err());
        let skeleton = snapshot("1701q-v2018-p7.9.6.0", ClosedReviewStatus::Skeleton);
        assert!(require_allowed_review_state(&skeleton).is_err());
        let candidate = snapshot("1701q-v2018-p7.9.6.0", ClosedReviewStatus::Candidate);
        assert!(require_allowed_review_state(&candidate).is_ok());
    }

    #[test]
    fn candidate_requires_a_real_pin_while_handoff_remains_planned() {
        let manifest = reviewed_manifest();
        let mut candidate = ClosedIndexSnapshot {
            rule_set_id: manifest.rule_set_id.clone(),
            form_code: manifest.form_code.clone(),
            form_revision: manifest.form_revision.clone(),
            official_package_version: manifest.official_package_version.clone(),
            source_set_sha256: Some("a".repeat(64)),
            path: format!("{}/rule-set.json", manifest.rule_set_id),
            review_status: ClosedReviewStatus::Candidate,
            profile_states: ClosedProfileStates {
                official: ClosedBranchState::Unresolved,
                filing_safe: ClosedBranchState::Unresolved,
            },
        };
        validate_closed_snapshot_identity(&candidate, &manifest)
            .expect("pinned candidate identity is satisfiable");

        candidate.source_set_sha256 = None;
        let error = validate_closed_snapshot_identity(&candidate, &manifest)
            .expect_err("candidate null pin must fail");
        assert!(
            error
                .to_string()
                .contains("planned/null state belongs only")
        );
        assert!(
            skeleton_handoff(&manifest)
                .identity
                .source_set_sha256
                .is_none(),
            "reviewed packet handoff remains planned/null"
        );
    }

    #[test]
    fn synthetic_private_candidate_copy_reaches_successful_proposed_audit() {
        let repo = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("canonicalize repository fixture");
        let rules_root = repo.join("rules");
        let source_root = rules_root.join("ir/v2");
        let current_tree = read_tree(&source_root).expect("read canonical v2 fixture");
        let index = parse_index(
            current_tree
                .get(INDEX_PATH)
                .expect("canonical fixture index"),
            &source_root.join(INDEX_PATH),
        )
        .expect("parse canonical candidate index");
        let protected = index
            .snapshots
            .iter()
            .find(|snapshot| snapshot.rule_set_id == PROTECTED_2550Q_RULE_SET_ID)
            .cloned()
            .expect("protected candidate fixture");
        let sequence = PROPOSAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let staging_label = std::env::temp_dir().join(format!(
            "bir-form-integration-synthetic-candidate-audit-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&staging_label).expect("create synthetic staging label");

        let (report, generation, _) = validate_external_proposal(
            &repo,
            &staging_label,
            &rules_root,
            &current_tree,
            &current_tree,
            PROTECTED_2550Q_RULE_SET_ID,
            &protected,
        )
        .expect("synthetic private candidate copy must pass proposed audit/generation");
        assert_eq!(report.snapshot_count(), 1);
        let audited = report.snapshots.first().expect("one audited candidate");
        assert_eq!(
            audited.document.review_status,
            crate::model::ReviewStatus::Candidate
        );
        assert!(
            audited.index.source_set_sha256.is_some(),
            "candidate aggregate audit requires a real source-set pin"
        );
        assert_eq!(generation.candidate_snapshot_count, 1);
        fs::remove_dir(staging_label).expect("remove synthetic staging label");
    }

    #[test]
    fn executable_filing_safe_branch_is_rejected_at_any_depth() {
        let value = parse_strict(
            br#"{"nested":[{"filing_safe":{"state":"executable"}}]}"#,
            Path::new("filing-safe-test.json"),
        )
        .expect("parse test JSON");
        let error = reject_executable_filing_safe_json(&value, "$")
            .expect_err("filing-safe execution must fail");
        assert!(error.to_string().contains("$.nested[0].filing_safe"));
    }

    #[test]
    fn staging_extras_are_rejected_by_name_before_any_file_capture() {
        let sequence = PROPOSAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "bir-form-integration-staging-allowlist-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create staging allowlist root");
        let (form_tree, schema_tree) = staging_layout_fixture(&root);
        fs::write(
            root.join("private-taxpayer-values.json"),
            b"must never be read",
        )
        .expect("write forbidden extra");

        let error =
            capture_staging_workspace(&root, "form-a", "form-a-p7.9.6.0", &form_tree, &schema_tree)
                .err()
                .expect("unexpected staging name must fail before capture");
        assert!(error.to_string().contains("closed allowlist"));
        assert!(error.to_string().contains("private-taxpayer-values.json"));
        fs::remove_dir_all(root).expect("remove staging allowlist root");
    }

    #[test]
    fn final_staging_epoch_recheck_detects_allowlisted_byte_mutation() {
        let sequence = PROPOSAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "bir-form-integration-staging-epoch-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create staging epoch root");
        let (form_tree, schema_tree) = staging_layout_fixture(&root);
        let first =
            capture_staging_workspace(&root, "form-a", "form-a-p7.9.6.0", &form_tree, &schema_tree)
                .expect("capture first staging epoch");
        fs::write(root.join(HANDOFF_PATH), b"{\"epoch\":2}\n").expect("mutate handoff epoch");
        let second =
            capture_staging_workspace(&root, "form-a", "form-a-p7.9.6.0", &form_tree, &schema_tree)
                .expect("capture second staging epoch");

        let error = compare_trees(
            "final staging epoch",
            &first.workspace_tree,
            &second.workspace_tree,
        )
        .expect_err("different staging epochs must not be mixed");
        assert!(error.to_string().contains(HANDOFF_PATH));
        fs::remove_dir_all(root).expect("remove staging epoch root");
    }

    #[test]
    fn proposal_cleanup_requires_the_exact_owner_marker() {
        let sequence = PROPOSAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "bir-form-integration-owner-test-{}-{sequence}",
            std::process::id()
        ));
        let staging = root.join("staging");
        fs::create_dir_all(&staging).expect("create owner test staging root");
        let mut workspace = ProposalWorkspace::create(&staging).expect("create proposal");
        let container = workspace.container.clone();
        let owner_path = container.join(PROPOSAL_OWNER_FILE);
        fs::write(&owner_path, b"tampered").expect("tamper owner marker");

        let error = workspace
            .cleanup()
            .expect_err("changed owner marker must block deletion");
        assert!(error.to_string().contains("owner marker changed"));
        assert!(container.exists(), "unowned tree must remain untouched");

        fs::write(&owner_path, &workspace.owner_marker).expect("restore owner marker");
        workspace.cleanup().expect("clean owned proposal");
        assert!(!container.exists());
        fs::remove_dir_all(root).expect("remove owner test root");
    }

    #[test]
    fn proposal_cleanup_rejects_a_replacement_with_a_copied_owner_marker() {
        let sequence = PROPOSAL_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "bir-form-integration-identity-test-{}-{sequence}",
            std::process::id()
        ));
        let staging = root.join("staging");
        fs::create_dir_all(&staging).expect("create identity test staging root");
        let mut workspace = ProposalWorkspace::create(&staging).expect("create proposal");
        let container = workspace.container.clone();
        let displaced = root.join("displaced-proposal");
        fs::rename(&container, &displaced).expect("displace owned proposal");
        fs::create_dir(&container).expect("create attacker replacement");
        fs::write(container.join(PROPOSAL_OWNER_FILE), &workspace.owner_marker)
            .expect("copy readable owner marker");
        let sentinel = container.join("sentinel.txt");
        fs::write(&sentinel, b"must survive").expect("write replacement sentinel");

        let error = workspace
            .cleanup()
            .expect_err("copied marker must not substitute for directory identity");
        assert!(error.to_string().contains("replaced"));
        assert_eq!(fs::read(&sentinel).expect("read sentinel"), b"must survive");

        fs::remove_dir_all(&container).expect("remove test replacement");
        fs::rename(&displaced, &container).expect("restore owned proposal path");
        workspace.cleanup().expect("clean restored owned proposal");
        fs::remove_dir_all(root).expect("remove identity test root");
    }

    #[test]
    fn self_authored_handoff_cannot_replace_reviewed_packet_identity() {
        let manifest = reviewed_manifest();
        let mut handoff = skeleton_handoff(&manifest);
        validate_handoff_artifact_binding(
            &handoff,
            &manifest,
            &manifest.tracked_v1_source_set_sha256,
        )
        .expect("exact reviewed artifact binding");

        handoff.packet.packet_digest_sha256 = "f".repeat(64);
        let error = validate_handoff_artifact_binding(
            &handoff,
            &manifest,
            &manifest.tracked_v1_source_set_sha256,
        )
        .expect_err("handoff-authored packet digest must not suffice");
        assert!(
            error
                .to_string()
                .contains("independently verified artifact")
        );
    }

    #[test]
    fn reviewed_ledger_digest_is_an_independent_required_binding() {
        let manifest = reviewed_manifest();
        let entry = reviewed_ledger_entry(&manifest);
        let mut ledger = ClosedReviewLedger {
            format: super::EVIDENCE_REVIEW_LEDGER_FORMAT.to_owned(),
            canonicalization: CANONICALIZATION_ID.to_owned(),
            entries: vec![entry],
        };
        validate_review_ledger_binding(&ledger, &manifest, &manifest.packet_digest_sha256)
            .expect("exact ledger binding");

        ledger.entries[0].expected_packet_digest_sha256 = Some("f".repeat(64));
        let error =
            validate_review_ledger_binding(&ledger, &manifest, &manifest.packet_digest_sha256)
                .expect_err("self-authored stale ledger digest must fail");
        assert!(error.to_string().contains("expected_packet_digest_sha256"));
    }

    #[test]
    fn reviewed_summary_requires_exact_source_map_and_verification_digests() {
        let manifest = reviewed_manifest();
        let ledger = reviewed_ledger_entry(&manifest);
        let provenance_sha256 = sha256_hex(
            &canonical_serialize(&ledger.capture_provenance, "test capture provenance")
                .expect("serialize capture provenance"),
        );
        let summary = json!({
            "format": EVIDENCE_SUMMARY_FORMAT,
            "canonicalization": CANONICALIZATION_ID,
            "form_id": manifest.form_id.clone(),
            "tracked_v1_source_set_sha256": manifest.tracked_v1_source_set_sha256.clone(),
            "capture_sessions": [{
                "capture_session_id": ledger.capture_session_id.clone(),
                "source_map_sha256": "3".repeat(64),
                "source_verification_sha256": "4".repeat(64),
                "capture_provenance_sha256": provenance_sha256,
                "upstream_evidence_ids": [manifest.official_package_evidence_id.clone()]
            }],
            "upstream_assets": [{
                "asset_id": ledger.official_package_asset_id.clone(),
                "kind": "official-package-executable",
                "upstream_evidence_id": manifest.official_package_evidence_id.clone()
            }]
        });
        let exact_bytes = serde_json::to_vec(&summary).expect("serialize exact summary");
        validate_review_summary_binding(&exact_bytes, &manifest, &ledger)
            .expect("exact capture-session digests must bind");

        let mut mismatched_ledger = ledger.clone();
        mismatched_ledger.source_map_sha256 = "5".repeat(64);
        let error = validate_review_summary_binding(&exact_bytes, &manifest, &mismatched_ledger)
            .expect_err("review-ledger source-map digest mismatch must fail");
        assert!(error.to_string().contains("capture session/provenance"));

        let mut missing = summary.clone();
        missing["capture_sessions"][0]
            .as_object_mut()
            .expect("capture session object")
            .remove("source_map_sha256");
        let error = validate_review_summary_binding(
            &serde_json::to_vec(&missing).expect("serialize missing digest"),
            &manifest,
            &ledger,
        )
        .expect_err("missing source-map digest must fail");
        assert!(error.to_string().contains("source_map_sha256"));

        let mut malformed = summary;
        malformed["capture_sessions"][0]["source_verification_sha256"] = json!("A".repeat(64));
        let error = validate_review_summary_binding(
            &serde_json::to_vec(&malformed).expect("serialize malformed digest"),
            &manifest,
            &ledger,
        )
        .expect_err("non-lowercase verification digest must fail");
        assert!(error.to_string().contains("source_verification_sha256"));
    }

    #[test]
    fn uncooperative_mutation_is_never_overwritten_without_atomic_directory_cas() {
        let sequence = PROPOSAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "bir-form-integration-cas-refusal-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create lock test root");
        let target = root.join("v2");
        let expected = BTreeMap::from([(INDEX_PATH.to_owned(), b"expected".to_vec())]);
        let concurrent = BTreeMap::from([(INDEX_PATH.to_owned(), b"uncooperative".to_vec())]);
        write_tree_atomically(&target, &expected).expect("write expected tree");
        write_tree_atomically(&target, &concurrent).expect("inject uncooperative mutation");

        let error = super::refuse_non_atomic_apply::<()>(&target, &expected)
            .expect_err("non-cooperative mutation must fail closed");
        assert!(
            error
                .to_string()
                .contains("changed outside form integration")
        );
        assert_eq!(
            read_tree(&target).expect("read untouched concurrent tree"),
            concurrent
        );
        fs::remove_dir_all(root).expect("remove CAS refusal test root");
    }

    #[cfg(unix)]
    #[test]
    fn proposal_cleanup_unlinks_attacker_symlink_without_following_it() {
        use std::os::unix::fs::symlink;

        let sequence = PROPOSAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "bir-form-integration-cleanup-symlink-test-{}-{sequence}",
            std::process::id()
        ));
        let staging = root.join("staging");
        let sentinel = root.join("sentinel");
        fs::create_dir_all(&staging).expect("create staging");
        fs::create_dir_all(&sentinel).expect("create sentinel");
        fs::write(sentinel.join("must-survive"), b"owned elsewhere").expect("write sentinel");

        let mut workspace = ProposalWorkspace::create(&staging).expect("create proposal");
        fs::create_dir_all(workspace.repo_root()).expect("create proposal repo");
        symlink(&sentinel, workspace.repo_root().join("attacker-link"))
            .expect("inject attacker symlink");
        let container = workspace.container.clone();
        workspace
            .cleanup()
            .expect("cleanup must unlink rather than follow symlink");

        assert!(!container.exists(), "owned proposal must be removed");
        assert!(
            sentinel.join("must-survive").is_file(),
            "symlink target must remain untouched"
        );
        fs::remove_dir_all(root).expect("remove symlink test root");
    }

    #[test]
    fn proposed_index_render_is_byte_stable_and_sorted_by_caller() {
        let index = ClosedIndex {
            schema: "../../schema/v2/index.schema.json".to_owned(),
            schema_version: "2.0.0".to_owned(),
            snapshots: vec![
                snapshot("1701q-v2018-p7.9.6.0", ClosedReviewStatus::Skeleton),
                snapshot(PROTECTED_2550Q_RULE_SET_ID, ClosedReviewStatus::Candidate),
            ],
        };
        let first = render_index(&index).expect("render index");
        let second = render_index(&index).expect("render index again");
        compare_trees(
            "stable render",
            &BTreeMap::from([(INDEX_PATH.to_owned(), first)]),
            &BTreeMap::from([(INDEX_PATH.to_owned(), second)]),
        )
        .expect("byte stable");
        let parsed: serde_json::Value =
            serde_json::from_slice(&render_index(&index).expect("render JSON"))
                .expect("valid JSON");
        assert_eq!(parsed["schema_version"], json!("2.0.0"));
    }
}
