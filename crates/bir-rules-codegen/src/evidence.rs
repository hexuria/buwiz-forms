//! Portable, fail-closed evidence packets for offline validation-rule work.
//!
//! A packet carries only reviewed, derived, value-free JSON. Original upstream
//! material remains in a separately controlled vault. The packet manifest
//! binds its review state, attestations, upstream metadata, derived file
//! metadata, and exact derived bytes with one domain-separated digest.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use same_file::Handle;
use serde::{Deserialize, Serialize};

use crate::audit::discover_default_repo_root;
use crate::error::{CodegenError, Result};
use crate::files::{
    ApprovedExternalRoot, ReadScope, read_external_bytes_under, read_external_tree_bound,
    read_external_tree_under, read_tracked_tree,
};
use crate::hash::{digest_entries, sha256_hex};
use crate::json::{CANONICALIZATION_ID, JsonValue, canonical_bytes, parse_strict};
use crate::path::{
    canonical_repo_root, discover_repo_root, ensure_under, is_same_or_below, is_same_path,
    is_symlink_or_reparse_point, portable_join, resolve_existing_under, validate_portable_relative,
};
use crate::sensitive::{
    looks_like_credential, looks_like_email, looks_like_online_submission, looks_like_tin,
};

pub const EVIDENCE_PACKET_FORMAT: &str = "bir-evidence-packet-v1";
pub const EVIDENCE_PACKET_MANIFEST: &str = "evidence-packet.json";
pub const EVIDENCE_PACKET_DIGEST_DOMAIN: &str = "bir-evidence-packet-digest-v1";
pub const STAGED_FORM_DIGEST_DOMAIN: &str = "bir-staged-form-v1";

const DERIVED_PREFIX: &str = "derived/";
const UPSTREAM_PREFIX: &str = "upstream/";
const DERIVED_MEDIA_TYPE: &str = "application/json";
const DERIVED_CLASSIFICATION: &str = "non-taxpayer-derived";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceReviewStatus {
    Candidate,
    Reviewed,
    Rejected,
}

impl EvidenceReviewStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Reviewed => "reviewed",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceAttestationKind {
    DerivedOnly,
    NoTaxpayerValues,
    NoCredentials,
    NoOnlineSubmission,
}

impl EvidenceAttestationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::DerivedOnly => "derived-only",
            Self::NoTaxpayerValues => "no-taxpayer-values",
            Self::NoCredentials => "no-credentials",
            Self::NoOnlineSubmission => "no-online-submission",
        }
    }
}

const REQUIRED_ATTESTATIONS: [EvidenceAttestationKind; 4] = [
    EvidenceAttestationKind::DerivedOnly,
    EvidenceAttestationKind::NoTaxpayerValues,
    EvidenceAttestationKind::NoCredentials,
    EvidenceAttestationKind::NoOnlineSubmission,
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DerivedEvidenceKind {
    SourceExcerpt,
    StructuredDomInventory,
    XmlInventory,
    RuntimeExactErrors,
    RuntimeValidationOrder,
    RuntimeSaveReopen,
    RecordCensus,
    GapReport,
}

impl DerivedEvidenceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::SourceExcerpt => "source-excerpt",
            Self::StructuredDomInventory => "structured-dom-inventory",
            Self::XmlInventory => "xml-inventory",
            Self::RuntimeExactErrors => "runtime-exact-errors",
            Self::RuntimeValidationOrder => "runtime-validation-order",
            Self::RuntimeSaveReopen => "runtime-save-reopen",
            Self::RecordCensus => "record-census",
            Self::GapReport => "gap-report",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub enum EvidenceObservation {
    Observed,
    NotObserved { reason: String },
    Gap { reason: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceCaptureOperatingSystem {
    Windows,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCaptureProvenance {
    pub tool_commit: String,
    pub command_argv: Vec<String>,
    pub capture_tool_version: String,
    pub operating_system: EvidenceCaptureOperatingSystem,
    pub windows_version: String,
    pub official_app_version: String,
    pub started_at_utc: String,
    pub finished_at_utc: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RuleSetSourceState {
    Planned { source_set_sha256: () },
    Pinned { source_set_sha256: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReview {
    pub status: EvidenceReviewStatus,
    pub reviewed_by: Option<String>,
    pub reviewed_at_utc: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceAttestation {
    pub kind: EvidenceAttestationKind,
    pub attested: bool,
    pub attested_by: String,
    pub attested_at_utc: String,
    pub statement: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamEvidenceFile {
    pub evidence_id: String,
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceExcerptLocator {
    pub upstream_evidence_id: String,
    pub full_file_path: String,
    pub full_file_size_bytes: u64,
    pub full_file_sha256: String,
    pub excerpt_start_byte: u64,
    pub excerpt_end_byte: u64,
    pub excerpt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedEvidenceFile {
    pub path: String,
    pub kind: DerivedEvidenceKind,
    pub observation: EvidenceObservation,
    pub source_excerpt: Option<SourceExcerptLocator>,
    pub media_type: String,
    pub classification: String,
    pub review_status: EvidenceReviewStatus,
    pub source_evidence_ids: Vec<String>,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePacketManifest {
    pub format: String,
    pub canonicalization: String,
    pub packet_id: String,
    pub form_id: String,
    pub rule_set_id: String,
    pub tracked_v1_source_set_sha256: String,
    pub rule_set_source_state: RuleSetSourceState,
    pub form_code: String,
    pub form_revision: String,
    pub official_package_version: String,
    pub official_package_evidence_id: String,
    pub source_map_sha256: String,
    pub source_verification_sha256: String,
    pub capture_provenance: EvidenceCaptureProvenance,
    pub created_at_utc: String,
    pub review: EvidenceReview,
    pub attestations: Vec<EvidenceAttestation>,
    pub upstream_evidence: Vec<UpstreamEvidenceFile>,
    pub derived_evidence: Vec<DerivedEvidenceFile>,
    pub packet_digest_sha256: String,
}

#[derive(Clone, Debug)]
pub struct VerifyEvidenceOptions {
    pub packet_dir: PathBuf,
    pub vault_dir: Option<PathBuf>,
}

impl VerifyEvidenceOptions {
    pub fn new(packet_dir: impl Into<PathBuf>) -> Self {
        Self {
            packet_dir: packet_dir.into(),
            vault_dir: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct VerifyEvidenceReport {
    pub packet_id: String,
    pub form_id: String,
    pub review_status: EvidenceReviewStatus,
    pub packet_digest_sha256: String,
    pub derived_file_count: usize,
    pub upstream_file_count: usize,
    pub full_upstream_verified: bool,
}

#[derive(Clone, Debug)]
pub struct ImportEvidenceOptions {
    pub packet_dir: PathBuf,
    pub staging_root: PathBuf,
    pub canonical_rules_dir: Option<PathBuf>,
}

impl ImportEvidenceOptions {
    pub fn new(packet_dir: impl Into<PathBuf>, staging_root: impl Into<PathBuf>) -> Self {
        Self {
            packet_dir: packet_dir.into(),
            staging_root: staging_root.into(),
            canonical_rules_dir: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportEvidenceReport {
    pub packet_id: String,
    pub form_id: String,
    pub packet_digest_sha256: String,
    pub imported_file_count: usize,
    pub staging_root: PathBuf,
}

#[derive(Clone, Debug)]
pub struct StageFormOptions {
    pub repo_root: PathBuf,
    pub form_id: String,
    pub staging_root: PathBuf,
    /// Optional reviewed packet that turns the mirror into a complete external
    /// one-form skeleton workspace. `None` preserves the original mirror-only
    /// behavior.
    pub packet_dir: Option<PathBuf>,
    pub(crate) read_scope: ReadScope,
}

impl StageFormOptions {
    pub fn tracked_checkout(
        repo_root: impl Into<PathBuf>,
        form_id: impl Into<String>,
        staging_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            form_id: form_id.into(),
            staging_root: staging_root.into(),
            packet_dir: None,
            read_scope: ReadScope::Tracked,
        }
    }

    pub fn external_workspace(
        repo_root: impl Into<PathBuf>,
        form_id: impl Into<String>,
        staging_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            form_id: form_id.into(),
            staging_root: staging_root.into(),
            packet_dir: None,
            read_scope: ReadScope::External,
        }
    }

    pub fn with_packet(mut self, packet_dir: impl Into<PathBuf>) -> Self {
        self.packet_dir = Some(packet_dir.into());
        self
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StageFormReport {
    pub form_id: String,
    pub staged_file_count: usize,
    pub staged_tree_sha256: String,
    pub staging_root: PathBuf,
    pub packet_id: Option<String>,
    pub rule_set_id: Option<String>,
    pub packet_digest_sha256: Option<String>,
}

pub(crate) struct VerifiedPacket {
    pub(crate) root: PathBuf,
    pub(crate) manifest: EvidencePacketManifest,
    pub(crate) derived_files: BTreeMap<String, Vec<u8>>,
    pub(crate) full_upstream_verified: bool,
}

/// Verifies a packet without writing anywhere.
///
/// A missing vault is not an error because a packet is designed to travel
/// independently of controlled upstream bytes. It is, however, always
/// reported as `full_upstream_verified: false`.
pub fn verify_evidence(options: &VerifyEvidenceOptions) -> Result<VerifyEvidenceReport> {
    let verified = verify_packet(options)?;
    Ok(report_for(&verified))
}

/// Verify a packet against bytes captured earlier through
/// `files::read_external_tree_bound`.
///
/// This is crate-private because callers must preserve the packet root/tree
/// binding. It exists for compound checks that must hash, parse, and verify one
/// exact snapshot without reopening packet files between those operations.
pub(crate) fn verify_evidence_from_tree(
    options: &VerifyEvidenceOptions,
    tree: &BTreeMap<String, Vec<u8>>,
) -> Result<VerifyEvidenceReport> {
    let packet_root = canonical_real_directory(&options.packet_dir, "evidence packet")?;
    let verified = verify_packet_tree(options, packet_root, tree)?;
    Ok(report_for(&verified))
}

/// Imports reviewed derived evidence into a brand-new staging root.
///
/// Candidate or rejected packets, candidate or rejected derived files,
/// existing targets, and canonical `rules/` targets are all refused.
pub fn import_evidence(options: &ImportEvidenceOptions) -> Result<ImportEvidenceReport> {
    let verified = verify_packet(&VerifyEvidenceOptions {
        packet_dir: options.packet_dir.clone(),
        vault_dir: None,
    })?;

    if verified.manifest.review.status != EvidenceReviewStatus::Reviewed {
        return Err(CodegenError::new(format!(
            "packet `{}` has review status `{}`; import requires `reviewed`",
            verified.manifest.packet_id,
            verified.manifest.review.status.as_str()
        )));
    }
    let unreviewed: Vec<&str> = verified
        .manifest
        .derived_evidence
        .iter()
        .filter(|file| file.review_status != EvidenceReviewStatus::Reviewed)
        .map(|file| file.path.as_str())
        .collect();
    if !unreviewed.is_empty() {
        return Err(CodegenError::new(format!(
            "packet `{}` has derived evidence that is not reviewed: {}",
            verified.manifest.packet_id,
            unreviewed.join(", ")
        )));
    }

    let staging_root = absolute_path(&options.staging_root)?;
    ensure_fresh_staging_target(
        &staging_root,
        "evidence import staging root",
        options.canonical_rules_dir.as_deref(),
        Some(&verified.root),
    )?;
    write_fresh_tree(&staging_root, &verified.derived_files)?;

    Ok(ImportEvidenceReport {
        packet_id: verified.manifest.packet_id,
        form_id: verified.manifest.form_id,
        packet_digest_sha256: verified.manifest.packet_digest_sha256,
        imported_file_count: verified.derived_files.len(),
        staging_root,
    })
}

/// Mirrors one canonical form directory into a brand-new staging root.
///
/// The mirror retains `rules/forms/<form-id>/...` beneath the staging root so
/// existing form tools can run against it with a different repository root.
pub fn stage_form(options: &StageFormOptions) -> Result<StageFormReport> {
    validate_identifier(&options.form_id, "form_id")?;
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let relative_form_root = format!("rules/forms/{}", options.form_id);
    let form_root =
        resolve_existing_under(&repo_root, &relative_form_root, "canonical form directory")?;
    let metadata = fs::metadata(&form_root)
        .map_err(|source| CodegenError::io("read canonical form metadata", &form_root, source))?;
    if !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "canonical form path `{}` is not a directory",
            form_root.display()
        )));
    }

    let source_files = match options.read_scope {
        ReadScope::Tracked => read_tracked_tree(&form_root)?,
        ReadScope::External => {
            let approved_repo =
                approve_exact_external_root(&repo_root, "external rules workspace", |_| Ok(()))?;
            read_external_tree_under(
                &approved_repo,
                &form_root,
                "canonical external form directory",
            )?
        }
    };
    if source_files.is_empty() {
        return Err(CodegenError::new(format!(
            "canonical form `{}` contains no files",
            options.form_id
        )));
    }
    let staging_root = absolute_path(&options.staging_root)?;
    ensure_fresh_staging_target(
        &staging_root,
        "form staging root",
        Some(&repo_root.join("rules")),
        Some(&form_root),
    )?;
    let (staged_files, packet_id, rule_set_id, packet_digest_sha256) =
        if let Some(packet_dir) = &options.packet_dir {
            let verified = verify_packet(&VerifyEvidenceOptions::new(packet_dir))?;
            ensure_fresh_staging_target(
                &staging_root,
                "packet-backed form staging root",
                Some(&repo_root.join("rules")),
                Some(&verified.root),
            )?;
            let plan = match options.read_scope {
                ReadScope::Tracked => crate::form_factory::build_packet_backed_form_plan(
                    &repo_root,
                    &form_root,
                    &options.form_id,
                    &verified,
                )?,
                ReadScope::External => crate::form_factory::build_packet_backed_form_plan_external(
                    &repo_root,
                    &form_root,
                    &options.form_id,
                    &verified,
                )?,
            };
            (
                plan.files,
                Some(plan.packet_id),
                Some(plan.rule_set_id),
                Some(plan.packet_digest_sha256),
            )
        } else {
            let mut staged_files = BTreeMap::new();
            for (relative, bytes) in source_files {
                staged_files.insert(format!("{relative_form_root}/{relative}"), bytes);
            }
            (staged_files, None, None, None)
        };
    write_fresh_tree(&staging_root, &staged_files)?;
    let staged_tree_sha256 = digest_entries(
        STAGED_FORM_DIGEST_DOMAIN,
        staged_files
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    );

    Ok(StageFormReport {
        form_id: options.form_id.clone(),
        staged_file_count: staged_files.len(),
        staged_tree_sha256,
        staging_root,
        packet_id,
        rule_set_id,
        packet_digest_sha256,
    })
}

fn verify_packet(options: &VerifyEvidenceOptions) -> Result<VerifiedPacket> {
    let packet_root = canonical_real_directory(&options.packet_dir, "evidence packet")?;
    let approved = approve_exact_external_root(&packet_root, "evidence packet", |_| Ok(()))?;
    let tree = read_external_tree_bound(&approved, "evidence packet")?;
    let verified = verify_packet_tree(options, packet_root, &tree)?;
    approved.revalidate("evidence packet")?;
    Ok(verified)
}

fn verify_packet_tree(
    options: &VerifyEvidenceOptions,
    packet_root: PathBuf,
    tree: &BTreeMap<String, Vec<u8>>,
) -> Result<VerifiedPacket> {
    let manifest_bytes = tree.get(EVIDENCE_PACKET_MANIFEST).ok_or_else(|| {
        CodegenError::new(format!(
            "evidence packet `{}` is missing `{EVIDENCE_PACKET_MANIFEST}`",
            packet_root.display()
        ))
    })?;
    let manifest_path = packet_root.join(EVIDENCE_PACKET_MANIFEST);
    let manifest = parse_canonical_manifest(manifest_bytes, &manifest_path)?;
    validate_manifest(&manifest)?;

    let declared_paths: BTreeSet<&str> = manifest
        .derived_evidence
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    let actual_paths: BTreeSet<&str> = tree
        .keys()
        .filter(|path| path.as_str() != EVIDENCE_PACKET_MANIFEST)
        .map(String::as_str)
        .collect();
    if actual_paths != declared_paths {
        let undeclared: Vec<&str> = actual_paths.difference(&declared_paths).copied().collect();
        let missing: Vec<&str> = declared_paths.difference(&actual_paths).copied().collect();
        return Err(CodegenError::new(format!(
            "packet file inventory differs from derived_evidence; undeclared=[{}] missing=[{}]",
            undeclared.join(", "),
            missing.join(", ")
        )));
    }

    let mut derived_files = BTreeMap::new();
    for declared in &manifest.derived_evidence {
        let path = resolve_existing_under(&packet_root, &declared.path, "derived evidence path")?;
        let metadata = fs::metadata(&path)
            .map_err(|source| CodegenError::io("read derived evidence metadata", &path, source))?;
        if !metadata.is_file() {
            return Err(CodegenError::new(format!(
                "derived evidence `{}` is not a regular file",
                path.display()
            )));
        }
        let bytes = tree
            .get(&declared.path)
            .expect("packet inventory equality proved every declared file is present");
        verify_size_and_hash(&declared.path, bytes, declared.size_bytes, &declared.sha256)?;
        let value = parse_canonical_json(bytes, &path, "derived evidence")?;
        reject_sensitive_derived_content(&value, &declared.path)?;
        derived_files.insert(declared.path.clone(), bytes.clone());
    }

    let computed_digest = packet_digest(&manifest, &derived_files)?;
    if computed_digest != manifest.packet_digest_sha256 {
        return Err(CodegenError::new(format!(
            "packet digest mismatch: manifest={} computed={computed_digest}",
            manifest.packet_digest_sha256
        )));
    }

    let full_upstream_verified = match &options.vault_dir {
        None => false,
        Some(vault_dir) => {
            verify_upstream_vault(
                vault_dir,
                &manifest.upstream_evidence,
                &manifest.derived_evidence,
            )?;
            true
        }
    };

    Ok(VerifiedPacket {
        root: packet_root,
        manifest,
        derived_files,
        full_upstream_verified,
    })
}

fn report_for(packet: &VerifiedPacket) -> VerifyEvidenceReport {
    VerifyEvidenceReport {
        packet_id: packet.manifest.packet_id.clone(),
        form_id: packet.manifest.form_id.clone(),
        review_status: packet.manifest.review.status,
        packet_digest_sha256: packet.manifest.packet_digest_sha256.clone(),
        derived_file_count: packet.manifest.derived_evidence.len(),
        upstream_file_count: packet.manifest.upstream_evidence.len(),
        full_upstream_verified: packet.full_upstream_verified,
    }
}

fn parse_canonical_manifest(bytes: &[u8], path: &Path) -> Result<EvidencePacketManifest> {
    let value = parse_canonical_json(bytes, path, "evidence packet manifest")?;
    serde_json::from_value(value.into_serde()).map_err(|source| {
        CodegenError::with_source(
            format!("closed-structure load of `{}` failed", path.display()),
            source,
        )
    })
}

fn parse_canonical_json(bytes: &[u8], path: &Path, label: &str) -> Result<JsonValue> {
    std::str::from_utf8(bytes).map_err(|source| {
        CodegenError::with_source(format!("{label} `{}` is not UTF-8", path.display()), source)
    })?;
    let value = parse_strict(bytes, path)?;
    let canonical = canonical_bytes(&value);
    if bytes != canonical {
        return Err(CodegenError::new(format!(
            "{label} `{}` is not canonical `{CANONICALIZATION_ID}` JSON",
            path.display()
        )));
    }
    Ok(value)
}

fn validate_manifest(manifest: &EvidencePacketManifest) -> Result<()> {
    if manifest.format != EVIDENCE_PACKET_FORMAT {
        return Err(CodegenError::new(format!(
            "unsupported evidence packet format `{}`; expected `{EVIDENCE_PACKET_FORMAT}`",
            manifest.format
        )));
    }
    if manifest.canonicalization != CANONICALIZATION_ID {
        return Err(CodegenError::new(format!(
            "unsupported packet canonicalization `{}`; expected `{CANONICALIZATION_ID}`",
            manifest.canonicalization
        )));
    }
    validate_identifier(&manifest.packet_id, "packet_id")?;
    validate_identifier(&manifest.form_id, "form_id")?;
    validate_identifier(&manifest.rule_set_id, "rule_set_id")?;
    validate_sha256(
        &manifest.tracked_v1_source_set_sha256,
        "tracked_v1_source_set_sha256",
        false,
    )?;
    if let RuleSetSourceState::Pinned { source_set_sha256 } = &manifest.rule_set_source_state {
        validate_sha256(
            source_set_sha256,
            "rule_set_source_state.source_set_sha256",
            false,
        )?;
    }
    validate_exact_identity(&manifest.form_code, "form_code")?;
    validate_exact_identity(&manifest.form_revision, "form_revision")?;
    validate_exact_identity(
        &manifest.official_package_version,
        "official_package_version",
    )?;
    validate_identifier(
        &manifest.official_package_evidence_id,
        "official_package_evidence_id",
    )?;
    validate_sha256(&manifest.source_map_sha256, "source_map_sha256", false)?;
    validate_sha256(
        &manifest.source_verification_sha256,
        "source_verification_sha256",
        false,
    )?;
    crate::vault_acquisition::validate_source_verifier_provenance(&manifest.capture_provenance)?;
    validate_utc_timestamp(&manifest.created_at_utc, "created_at_utc")?;
    validate_sha256(
        &manifest.packet_digest_sha256,
        "packet_digest_sha256",
        false,
    )?;
    validate_review(&manifest.review)?;
    validate_attestations(&manifest.attestations)?;

    if manifest.upstream_evidence.is_empty() {
        return Err(CodegenError::new(
            "upstream_evidence must name at least one vault-held source",
        ));
    }
    if manifest.derived_evidence.is_empty() {
        return Err(CodegenError::new(
            "derived_evidence must name at least one packet file",
        ));
    }

    let mut upstream_ids = BTreeSet::new();
    let mut upstream_paths = BTreeSet::new();
    let mut previous_upstream_id: Option<&str> = None;
    for upstream in &manifest.upstream_evidence {
        validate_identifier(&upstream.evidence_id, "upstream evidence_id")?;
        if previous_upstream_id.is_some_and(|previous| previous >= upstream.evidence_id.as_str()) {
            return Err(CodegenError::new(
                "upstream_evidence must be strictly ordered by evidence_id",
            ));
        }
        previous_upstream_id = Some(&upstream.evidence_id);
        if !upstream_ids.insert(upstream.evidence_id.as_str()) {
            return Err(CodegenError::new(format!(
                "duplicate upstream evidence_id `{}`",
                upstream.evidence_id
            )));
        }
        validate_packet_path(&upstream.path, UPSTREAM_PREFIX, "upstream evidence path")?;
        if !upstream_paths.insert(upstream.path.as_str()) {
            return Err(CodegenError::new(format!(
                "duplicate upstream evidence path `{}`",
                upstream.path
            )));
        }
        validate_sha256(
            &upstream.sha256,
            &format!("upstream `{}` sha256", upstream.evidence_id),
            false,
        )?;
    }
    if !upstream_ids.contains(manifest.official_package_evidence_id.as_str()) {
        return Err(CodegenError::new(format!(
            "official_package_evidence_id `{}` does not name declared upstream evidence",
            manifest.official_package_evidence_id
        )));
    }

    let mut previous_derived_path: Option<&str> = None;
    let mut derived_paths = BTreeSet::new();
    for derived in &manifest.derived_evidence {
        if previous_derived_path.is_some_and(|previous| previous >= derived.path.as_str()) {
            return Err(CodegenError::new(
                "derived_evidence must be strictly ordered by path",
            ));
        }
        previous_derived_path = Some(&derived.path);
        validate_packet_path(&derived.path, DERIVED_PREFIX, "derived evidence path")?;
        if !derived.path.ends_with(".json") {
            return Err(CodegenError::new(format!(
                "derived evidence path `{}` must end in `.json`",
                derived.path
            )));
        }
        if !derived_paths.insert(derived.path.as_str()) {
            return Err(CodegenError::new(format!(
                "duplicate derived evidence path `{}`",
                derived.path
            )));
        }
        if derived.media_type != DERIVED_MEDIA_TYPE {
            return Err(CodegenError::new(format!(
                "derived evidence `{}` media_type must be `{DERIVED_MEDIA_TYPE}`",
                derived.path
            )));
        }
        if derived.classification != DERIVED_CLASSIFICATION {
            return Err(CodegenError::new(format!(
                "derived evidence `{}` classification must be `{DERIVED_CLASSIFICATION}`",
                derived.path
            )));
        }
        validate_observation(&derived.observation, &derived.path)?;
        validate_sha256(
            &derived.sha256,
            &format!("derived evidence `{}` sha256", derived.path),
            false,
        )?;
        if derived.source_evidence_ids.is_empty() {
            return Err(CodegenError::new(format!(
                "derived evidence `{}` must cite at least one upstream evidence_id",
                derived.path
            )));
        }
        let mut previous_source: Option<&str> = None;
        for source_id in &derived.source_evidence_ids {
            if previous_source.is_some_and(|previous| previous >= source_id.as_str()) {
                return Err(CodegenError::new(format!(
                    "derived evidence `{}` source_evidence_ids must be strictly ordered",
                    derived.path
                )));
            }
            previous_source = Some(source_id);
            if !upstream_ids.contains(source_id.as_str()) {
                return Err(CodegenError::new(format!(
                    "derived evidence `{}` cites unknown upstream evidence_id `{source_id}`",
                    derived.path
                )));
            }
        }
        validate_source_excerpt(derived, &manifest.upstream_evidence)?;
    }
    Ok(())
}

fn validate_observation(observation: &EvidenceObservation, path: &str) -> Result<()> {
    match observation {
        EvidenceObservation::Observed => Ok(()),
        EvidenceObservation::NotObserved { reason } | EvidenceObservation::Gap { reason } => {
            validate_human_text(
                reason,
                &format!("derived evidence `{path}` observation reason"),
            )
        }
    }
}

fn validate_source_excerpt(
    derived: &DerivedEvidenceFile,
    upstream_evidence: &[UpstreamEvidenceFile],
) -> Result<()> {
    let locator = match (derived.kind, &derived.observation, &derived.source_excerpt) {
        (DerivedEvidenceKind::SourceExcerpt, EvidenceObservation::Observed, Some(locator)) => {
            locator
        }
        (DerivedEvidenceKind::SourceExcerpt, EvidenceObservation::Observed, None) => {
            return Err(CodegenError::new(format!(
                "observed source excerpt `{}` requires a full-file locator and excerpt hash",
                derived.path
            )));
        }
        (DerivedEvidenceKind::SourceExcerpt, _, None) => return Ok(()),
        (DerivedEvidenceKind::SourceExcerpt, _, Some(_)) => {
            return Err(CodegenError::new(format!(
                "unobserved source excerpt `{}` must not invent a locator",
                derived.path
            )));
        }
        (_, _, None) => return Ok(()),
        (_, _, Some(_)) => {
            return Err(CodegenError::new(format!(
                "derived evidence kind `{}` at `{}` must leave source_excerpt null",
                derived.kind.as_str(),
                derived.path
            )));
        }
    };

    validate_identifier(
        &locator.upstream_evidence_id,
        "source excerpt upstream_evidence_id",
    )?;
    validate_packet_path(
        &locator.full_file_path,
        UPSTREAM_PREFIX,
        "source excerpt full_file_path",
    )?;
    validate_sha256(
        &locator.full_file_sha256,
        "source excerpt full_file_sha256",
        false,
    )?;
    validate_sha256(
        &locator.excerpt_sha256,
        "source excerpt excerpt_sha256",
        false,
    )?;
    if locator.excerpt_start_byte >= locator.excerpt_end_byte
        || locator.excerpt_end_byte > locator.full_file_size_bytes
    {
        return Err(CodegenError::new(format!(
            "source excerpt `{}` byte range must be non-empty and within the full file",
            derived.path
        )));
    }
    if !derived
        .source_evidence_ids
        .iter()
        .any(|source_id| source_id == &locator.upstream_evidence_id)
    {
        return Err(CodegenError::new(format!(
            "source excerpt `{}` locator must be included in source_evidence_ids",
            derived.path
        )));
    }
    let upstream = upstream_evidence
        .iter()
        .find(|upstream| upstream.evidence_id == locator.upstream_evidence_id)
        .ok_or_else(|| {
            CodegenError::new(format!(
                "source excerpt `{}` cites unknown locator evidence_id `{}`",
                derived.path, locator.upstream_evidence_id
            ))
        })?;
    if locator.full_file_path != upstream.path
        || locator.full_file_size_bytes != upstream.size_bytes
        || locator.full_file_sha256 != upstream.sha256
    {
        return Err(CodegenError::new(format!(
            "source excerpt `{}` full-file locator does not exactly match upstream evidence `{}`",
            derived.path, locator.upstream_evidence_id
        )));
    }
    Ok(())
}

fn validate_review(review: &EvidenceReview) -> Result<()> {
    match review.status {
        EvidenceReviewStatus::Candidate => {
            if review.reviewed_by.is_some() || review.reviewed_at_utc.is_some() {
                return Err(CodegenError::new(
                    "candidate review must leave reviewed_by and reviewed_at_utc null",
                ));
            }
        }
        EvidenceReviewStatus::Reviewed | EvidenceReviewStatus::Rejected => {
            let reviewer = review.reviewed_by.as_deref().ok_or_else(|| {
                CodegenError::new("reviewed/rejected packet requires reviewed_by")
            })?;
            validate_human_text(reviewer, "reviewed_by")?;
            let timestamp = review.reviewed_at_utc.as_deref().ok_or_else(|| {
                CodegenError::new("reviewed/rejected packet requires reviewed_at_utc")
            })?;
            validate_utc_timestamp(timestamp, "reviewed_at_utc")?;
        }
    }
    Ok(())
}

fn validate_attestations(attestations: &[EvidenceAttestation]) -> Result<()> {
    if attestations.len() != REQUIRED_ATTESTATIONS.len() {
        return Err(CodegenError::new(format!(
            "packet requires exactly {} attestations",
            REQUIRED_ATTESTATIONS.len()
        )));
    }
    for (attestation, expected) in attestations.iter().zip(REQUIRED_ATTESTATIONS) {
        if attestation.kind != expected {
            return Err(CodegenError::new(format!(
                "attestations must be complete and ordered; expected `{}`, found `{}`",
                expected.as_str(),
                attestation.kind.as_str()
            )));
        }
        if !attestation.attested {
            return Err(CodegenError::new(format!(
                "attestation `{}` is not affirmed",
                attestation.kind.as_str()
            )));
        }
        validate_human_text(&attestation.attested_by, "attested_by")?;
        validate_utc_timestamp(&attestation.attested_at_utc, "attested_at_utc")?;
        validate_human_text(&attestation.statement, "attestation statement")?;
    }
    Ok(())
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    if value.is_empty() || value.len() > 128 {
        return Err(CodegenError::new(format!(
            "{label} must contain 1..=128 ASCII characters"
        )));
    }
    let mut characters = value.chars();
    let first = characters.next().expect("empty identifiers were rejected");
    if !(first.is_ascii_lowercase() || first.is_ascii_digit())
        || !characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_' | '.')
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

fn validate_exact_identity(value: &str, label: &str) -> Result<()> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > 256
        || value.chars().any(char::is_control)
        || looks_like_email(value)
        || looks_like_tin(value)
        || looks_like_credential(value)
        || looks_like_online_submission(value)
    {
        return Err(CodegenError::new(format!(
            "{label} must be exact, non-empty, trimmed, control-free, non-sensitive UTF-8 text"
        )));
    }
    Ok(())
}

fn validate_human_text(value: &str, label: &str) -> Result<()> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > 1_024
        || value.chars().any(char::is_control)
        || looks_like_email(value)
        || looks_like_tin(value)
        || looks_like_credential(value)
        || looks_like_online_submission(value)
    {
        return Err(CodegenError::new(format!(
            "{label} must be non-empty, trimmed, control-free, non-sensitive UTF-8 text"
        )));
    }
    Ok(())
}

fn validate_utc_timestamp(value: &str, label: &str) -> Result<()> {
    let bytes = value.as_bytes();
    let shape = bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7 | 10 | 13 | 16 | 19) || byte.is_ascii_digit()
        });
    if !shape {
        return Err(CodegenError::new(format!(
            "{label} `{value}` must be an exact UTC timestamp `YYYY-MM-DDTHH:MM:SSZ`"
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
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => 0,
    };
    if year == 0 || day == 0 || day > days_in_month || hour > 23 || minute > 59 || second > 59 {
        return Err(CodegenError::new(format!(
            "{label} `{value}` is not a real Gregorian UTC date and time"
        )));
    }
    Ok(())
}

fn validate_sha256(value: &str, label: &str, allow_empty: bool) -> Result<()> {
    if allow_empty && value.is_empty() {
        return Ok(());
    }
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

fn validate_packet_path(value: &str, prefix: &str, label: &str) -> Result<()> {
    validate_portable_relative(value, label)?;
    if !value.starts_with(prefix) || value.len() == prefix.len() {
        return Err(CodegenError::new(format!(
            "{label} `{value}` must be beneath `{prefix}`"
        )));
    }
    Ok(())
}

fn verify_size_and_hash(
    relative: &str,
    bytes: &[u8],
    expected_size: u64,
    expected_hash: &str,
) -> Result<()> {
    if bytes.len() as u64 != expected_size {
        return Err(CodegenError::new(format!(
            "`{relative}` size mismatch: manifest={expected_size} actual={}",
            bytes.len()
        )));
    }
    let actual_hash = sha256_hex(bytes);
    if actual_hash != expected_hash {
        return Err(CodegenError::new(format!(
            "`{relative}` SHA-256 mismatch: manifest={expected_hash} actual={actual_hash}"
        )));
    }
    Ok(())
}

fn packet_digest(
    manifest: &EvidencePacketManifest,
    derived_files: &BTreeMap<String, Vec<u8>>,
) -> Result<String> {
    let mut digest_manifest = manifest.clone();
    digest_manifest.packet_digest_sha256.clear();
    let manifest_bytes = canonical_serialize(&digest_manifest, "packet digest manifest")?;
    let mut entries = Vec::with_capacity(derived_files.len() + 1);
    entries.push((EVIDENCE_PACKET_MANIFEST.to_owned(), manifest_bytes));
    entries.extend(
        derived_files
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

fn canonical_serialize(value: &impl Serialize, label: &str) -> Result<Vec<u8>> {
    let ordinary = serde_json::to_vec(value)
        .map_err(|source| CodegenError::with_source(format!("serialize {label}"), source))?;
    let parsed = parse_strict(&ordinary, Path::new(label))?;
    Ok(canonical_bytes(&parsed))
}

fn verify_upstream_vault(
    vault_dir: &Path,
    files: &[UpstreamEvidenceFile],
    derived_files: &[DerivedEvidenceFile],
) -> Result<()> {
    let vault_root = canonical_real_directory(vault_dir, "upstream evidence vault")?;
    let approved_vault =
        approve_exact_external_root(&vault_root, "upstream evidence vault", |_| Ok(()))?;
    for declared in files {
        let path =
            resolve_existing_under(&vault_root, &declared.path, "upstream vault evidence path")?;
        let metadata = fs::metadata(&path)
            .map_err(|source| CodegenError::io("read upstream evidence metadata", &path, source))?;
        if !metadata.is_file() {
            return Err(CodegenError::new(format!(
                "upstream evidence `{}` is not a regular file",
                path.display()
            )));
        }
        let bytes =
            read_bound_external_file(&path, &approved_vault, "upstream vault evidence file")?;
        verify_size_and_hash(
            &declared.path,
            &bytes,
            declared.size_bytes,
            &declared.sha256,
        )?;
        for locator in derived_files
            .iter()
            .filter_map(|derived| derived.source_excerpt.as_ref())
            .filter(|locator| locator.upstream_evidence_id == declared.evidence_id)
        {
            let start = usize::try_from(locator.excerpt_start_byte).map_err(|_| {
                CodegenError::new(format!(
                    "source excerpt start offset does not fit this platform: {}",
                    locator.excerpt_start_byte
                ))
            })?;
            let end = usize::try_from(locator.excerpt_end_byte).map_err(|_| {
                CodegenError::new(format!(
                    "source excerpt end offset does not fit this platform: {}",
                    locator.excerpt_end_byte
                ))
            })?;
            let excerpt = bytes.get(start..end).ok_or_else(|| {
                CodegenError::new(format!(
                    "source excerpt byte range {start}..{end} is outside `{}`",
                    declared.path
                ))
            })?;
            let actual_hash = sha256_hex(excerpt);
            if actual_hash != locator.excerpt_sha256 {
                return Err(CodegenError::new(format!(
                    "source excerpt SHA-256 mismatch for `{}` bytes {start}..{end}: manifest={} actual={actual_hash}",
                    declared.path, locator.excerpt_sha256
                )));
            }
        }
    }
    Ok(())
}

fn read_bound_external_file(
    path: &Path,
    approved_root: &ApprovedExternalRoot,
    label: &str,
) -> Result<Vec<u8>> {
    read_external_bytes_under(approved_root, path, label)
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

fn reject_sensitive_derived_content(value: &JsonValue, relative: &str) -> Result<()> {
    inspect_derived_value(value, relative, "$")
}

fn inspect_derived_value(value: &JsonValue, relative: &str, location: &str) -> Result<()> {
    match value {
        JsonValue::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                inspect_derived_value(value, relative, &format!("{location}[{index}]"))?;
            }
        }
        JsonValue::Object(values) => {
            for (key, value) in values {
                let normalized: String = key
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect();
                if forbidden_derived_key(&normalized) {
                    return Err(CodegenError::new(format!(
                        "derived evidence `{relative}` contains forbidden sensitive/transport key `{key}` at {location}"
                    )));
                }
                inspect_derived_value(value, relative, &format!("{location}.{key}"))?;
            }
        }
        JsonValue::String(text) => {
            if looks_like_email(text)
                || looks_like_tin(text)
                || looks_like_credential(text)
                || looks_like_online_submission(text)
            {
                return Err(CodegenError::new(format!(
                    "derived evidence `{relative}` contains a credential, taxpayer value, or online-submission value at {location}"
                )));
            }
        }
        JsonValue::Number(number) => {
            if number.as_u64().is_some_and(|number| {
                (100_000_000..=999_999_999).contains(&number)
                    || (100_000_000_000..=999_999_999_999).contains(&number)
            }) {
                return Err(CodegenError::new(format!(
                    "derived evidence `{relative}` contains a taxpayer-shaped numeric value at {location}"
                )));
            }
        }
        JsonValue::Null | JsonValue::Bool(_) => {}
    }
    Ok(())
}

fn forbidden_derived_key(normalized: &str) -> bool {
    matches!(
        normalized,
        "rawvalue"
            | "rawvalues"
            | "rawinput"
            | "rawinputs"
            | "canonicalinput"
            | "canonicalinputs"
            | "fieldvalue"
            | "fieldvalues"
            | "taxpayer"
            | "taxpayervalue"
            | "taxpayername"
            | "taxpayeraddress"
            | "taxpayeremail"
            | "taxpayerphone"
            | "tin"
            | "taxpayeridentificationnumber"
            | "password"
            | "passwd"
            | "secret"
            | "credential"
            | "credentials"
            | "username"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "authorization"
            | "apikey"
            | "clientsecret"
            | "payload"
            | "request"
            | "response"
            | "body"
            | "submission"
            | "submissionpayload"
            | "submissionrequest"
            | "submitonline"
            | "onlinesubmission"
            | "transport"
            | "transmit"
            | "endpoint"
            | "queue"
    )
}

fn canonical_real_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| CodegenError::io(&format!("inspect {label}"), path, source))?;
    if is_symlink_or_reparse_point(&metadata) {
        return Err(CodegenError::new(format!(
            "{label} `{}` must not be a symlink or reparse point",
            path.display()
        )));
    }
    if !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "{label} `{}` is not a directory",
            path.display()
        )));
    }
    fs::canonicalize(path)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label}"), path, source))
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
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

fn ensure_fresh_staging_target(
    target: &Path,
    label: &str,
    canonical_rules_dir: Option<&Path>,
    source_root: Option<&Path>,
) -> Result<()> {
    if target.exists() {
        return Err(CodegenError::new(format!(
            "{label} `{}` already exists; refusing to overwrite",
            target.display()
        )));
    }
    let parent = target.parent().ok_or_else(|| {
        CodegenError::new(format!("{label} `{}` has no parent", target.display()))
    })?;
    if target.file_name().is_none() {
        return Err(CodegenError::new(format!(
            "{label} `{}` has no final path component",
            target.display()
        )));
    }
    reject_existing_symlink_ancestors(target, label)?;

    if let Some(rules_dir) = canonical_rules_dir {
        let rules_dir = fs::canonicalize(rules_dir).map_err(|source| {
            CodegenError::io("canonicalize canonical rules directory", rules_dir, source)
        })?;
        if is_same_or_below(&rules_dir, target) {
            return Err(CodegenError::new(format!(
                "{label} `{}` is inside canonical rules `{}`",
                target.display(),
                rules_dir.display()
            )));
        }
    }
    if let Some(source_root) = source_root
        && is_same_or_below(source_root, target)
    {
        return Err(CodegenError::new(format!(
            "{label} `{}` is inside its source `{}`",
            target.display(),
            source_root.display()
        )));
    }
    let mut existing = parent;
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| {
            CodegenError::new(format!(
                "{label} `{}` has no existing ancestor",
                target.display()
            ))
        })?;
    }
    let resolved = fs::canonicalize(existing).map_err(|source| {
        CodegenError::io(&format!("canonicalize {label} ancestor"), existing, source)
    })?;
    if let Some(source_root) = source_root
        && is_same_or_below(source_root, &resolved)
    {
        return Err(CodegenError::new(format!(
            "{label} `{}` resolves inside its source `{}`",
            target.display(),
            source_root.display()
        )));
    }
    if let Some(rules_dir) = canonical_rules_dir {
        let rules_dir = fs::canonicalize(rules_dir).map_err(|source| {
            CodegenError::io("canonicalize canonical rules directory", rules_dir, source)
        })?;
        if is_same_or_below(&rules_dir, &resolved) {
            return Err(CodegenError::new(format!(
                "{label} `{}` resolves beneath canonical rules `{}`",
                target.display(),
                rules_dir.display()
            )));
        }
    }
    // The public API may be used without an explicitly supplied repository.
    // If the target itself is beneath a recognizable checkout, discover that
    // checkout from the existing ancestor and close its canonical rules root
    // anyway.
    if let Ok(repo_root) = discover_repo_root(existing) {
        let rules_dir = repo_root.join("rules");
        if is_same_or_below(&rules_dir, &resolved) || is_same_or_below(&rules_dir, target) {
            return Err(CodegenError::new(format!(
                "{label} `{}` is inside canonical rules `{}`",
                target.display(),
                rules_dir.display()
            )));
        }
    }
    Ok(())
}

fn reject_existing_symlink_ancestors(path: &Path, label: &str) -> Result<()> {
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
    Ok(())
}

fn write_fresh_tree(target: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let parent = target.parent().ok_or_else(|| {
        CodegenError::new(format!(
            "fresh output `{}` has no parent directory",
            target.display()
        ))
    })?;
    fs::create_dir_all(parent)
        .map_err(|source| CodegenError::io("create fresh-output parent", parent, source))?;
    let canonical_parent = canonical_real_directory(parent, "fresh-output parent")?;
    let parent_identity = Handle::from_path(&canonical_parent)
        .map_err(|source| CodegenError::io("identify fresh-output parent", parent, source))?;
    fs::create_dir(target).map_err(|source| {
        if source.kind() == std::io::ErrorKind::AlreadyExists {
            CodegenError::new(format!(
                "fresh output `{}` already exists; refusing to overwrite",
                target.display()
            ))
        } else {
            CodegenError::io("create fresh output", target, source)
        }
    })?;
    let target_identity = Handle::from_path(target)
        .map_err(|source| CodegenError::io("identify fresh output root", target, source))?;
    let canonical_target = fs::canonicalize(target)
        .map_err(|source| CodegenError::io("canonicalize fresh output root", target, source))?;
    if canonical_target
        .parent()
        .is_none_or(|observed| !is_same_path(observed, &canonical_parent))
    {
        return Err(CodegenError::new(
            "fresh output was created outside its verified parent; it was left in place for manual inspection",
        ));
    }

    let write_result = write_fresh_tree_contents(
        target,
        &canonical_target,
        &target_identity,
        &canonical_parent,
        &parent_identity,
        files,
    );
    if let Err(write_error) = write_result {
        return Err(CodegenError::new(format!(
            "{write_error}; incomplete fresh output `{}` was left in place to avoid deleting a concurrently substituted path",
            target.display()
        )));
    }
    require_path_identity(target, &target_identity, "fresh output root")?;
    let approved_output = approve_exact_external_root(target, "fresh output root", |_| Ok(()))
        .map_err(|error| {
            CodegenError::new(format!(
                "{error}; fresh output `{}` was left in place after post-write approval failed",
                target.display()
            ))
        })?;
    let observed =
        read_external_tree_bound(&approved_output, "fresh output root").map_err(|error| {
            CodegenError::new(format!(
                "{error}; fresh output `{}` was left in place after post-write verification failed",
                target.display()
            ))
        })?;
    if &observed != files {
        return Err(CodegenError::new(format!(
            "fresh output `{}` failed its exact post-write tree comparison and was left in place for inspection",
            target.display()
        )));
    }
    require_path_identity(target, &target_identity, "fresh output root")?;
    require_path_identity(&canonical_parent, &parent_identity, "fresh-output parent")?;
    Ok(())
}

fn write_fresh_tree_contents(
    target: &Path,
    canonical_target: &Path,
    target_identity: &Handle,
    canonical_parent: &Path,
    parent_identity: &Handle,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    for (relative, bytes) in files {
        require_path_identity(target, target_identity, "fresh output root")?;
        require_path_identity(canonical_parent, parent_identity, "fresh-output parent")?;
        let path = portable_join(target, relative, "fresh output file path")?;
        ensure_under(target, &path, "fresh output file")?;
        let parent = path
            .parent()
            .expect("portable relative file path always has a parent");
        fs::create_dir_all(parent)
            .map_err(|source| CodegenError::io("create fresh output directory", parent, source))?;
        reject_existing_symlink_ancestors(&path, "fresh output file")?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|source| CodegenError::io("create fresh output file", &path, source))?;
        file.write_all(bytes)
            .map_err(|source| CodegenError::io("write fresh output file", &path, source))?;
        file.sync_all()
            .map_err(|source| CodegenError::io("sync fresh output file", &path, source))?;
        let created_identity = Handle::from_file(file).map_err(|source| {
            CodegenError::io("identify created fresh output file", &path, source)
        })?;
        let current_identity = Handle::from_path(&path)
            .map_err(|source| CodegenError::io("reidentify fresh output file", &path, source))?;
        if current_identity != created_identity {
            return Err(CodegenError::new(format!(
                "fresh output file `{}` was replaced while it was being written",
                path.display()
            )));
        }
        let canonical_path = fs::canonicalize(&path)
            .map_err(|source| CodegenError::io("canonicalize fresh output file", &path, source))?;
        ensure_under(
            canonical_target,
            &canonical_path,
            "canonical fresh output file",
        )?;
        require_path_identity(target, target_identity, "fresh output root")?;
        require_path_identity(canonical_parent, parent_identity, "fresh-output parent")?;
    }
    sync_directory(target)
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
            "sync fresh output directory",
            path,
            source,
        )),
    }
}

/// Minimal command parser kept outside `main.rs` so the existing codegen CLI
/// only needs one early dispatch branch.
pub fn run_evidence_command(
    command: &str,
    arguments: impl IntoIterator<Item = String>,
) -> Result<()> {
    let mut packet = None;
    let mut vault = None;
    let mut staging_root = None;
    let mut form_id = None;
    let mut repo_root = None;
    let mut help = false;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--packet"
                if matches!(
                    command,
                    "verify-evidence" | "import-evidence" | "stage-form"
                ) =>
            {
                set_once(
                    &mut packet,
                    next_cli_value(&mut arguments, "--packet")?,
                    "--packet",
                )?;
            }
            "--vault" if command == "verify-evidence" => {
                set_once(
                    &mut vault,
                    next_cli_value(&mut arguments, "--vault")?,
                    "--vault",
                )?;
            }
            "--staging-root" if matches!(command, "import-evidence" | "stage-form") => {
                set_once(
                    &mut staging_root,
                    next_cli_value(&mut arguments, "--staging-root")?,
                    "--staging-root",
                )?;
            }
            "--form-id" if command == "stage-form" => {
                set_once(
                    &mut form_id,
                    next_cli_value(&mut arguments, "--form-id")?,
                    "--form-id",
                )?;
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
                    evidence_usage(command)
                )));
            }
        }
    }
    if help {
        println!("{}", evidence_usage(command));
        return Ok(());
    }

    match command {
        "verify-evidence" => {
            if repo_root.is_some() {
                return Err(CodegenError::new(
                    "`verify-evidence` does not accept --repo-root",
                ));
            }
            let packet = required_cli_path(packet, "--packet")?;
            let mut options = VerifyEvidenceOptions::new(packet);
            options.vault_dir = vault.map(PathBuf::from);
            let report = verify_evidence(&options)?;
            println!(
                "verified packet {} for {}: {} derived file(s), {} upstream reference(s), review={}",
                report.packet_id,
                report.form_id,
                report.derived_file_count,
                report.upstream_file_count,
                report.review_status.as_str()
            );
            println!("packet digest: {}", report.packet_digest_sha256);
            println!("full upstream verified: {}", report.full_upstream_verified);
        }
        "import-evidence" => {
            if vault.is_some() || form_id.is_some() {
                return Err(CodegenError::new(
                    "`import-evidence` does not accept vault/form options",
                ));
            }
            let packet = required_cli_path(packet, "--packet")?;
            let staging_root = required_cli_path(staging_root, "--staging-root")?;
            let repo_root = match repo_root {
                Some(path) => PathBuf::from(path),
                None => discover_default_repo_root()?,
            };
            let repo_root = canonical_repo_root(&repo_root)?;
            let mut options = ImportEvidenceOptions::new(packet, staging_root);
            options.canonical_rules_dir = Some(repo_root.join("rules"));
            let report = import_evidence(&options)?;
            println!(
                "imported {} reviewed derived file(s) from packet {} for {} into {}",
                report.imported_file_count,
                report.packet_id,
                report.form_id,
                report.staging_root.display()
            );
            println!("packet digest: {}", report.packet_digest_sha256);
        }
        "stage-form" => {
            if vault.is_some() {
                return Err(CodegenError::new("`stage-form` does not accept --vault"));
            }
            let form_id =
                form_id.ok_or_else(|| CodegenError::new("`stage-form` requires --form-id <id>"))?;
            let staging_root = required_cli_path(staging_root, "--staging-root")?;
            let repo_root = match repo_root {
                Some(path) => PathBuf::from(path),
                None => discover_default_repo_root()?,
            };
            let mut options = StageFormOptions::tracked_checkout(repo_root, form_id, staging_root);
            options.packet_dir = packet.map(PathBuf::from);
            let report = stage_form(&options)?;
            println!(
                "staged {} file(s) for {} into {}",
                report.staged_file_count,
                report.form_id,
                report.staging_root.display()
            );
            println!("staged tree digest: {}", report.staged_tree_sha256);
            if let (Some(packet_id), Some(rule_set_id), Some(packet_digest)) = (
                report.packet_id.as_deref(),
                report.rule_set_id.as_deref(),
                report.packet_digest_sha256.as_deref(),
            ) {
                println!(
                    "packet-backed skeleton: packet={packet_id} rule_set={rule_set_id} digest={packet_digest}"
                );
            }
        }
        _ => {
            return Err(CodegenError::new(format!(
                "unknown evidence command `{command}`"
            )));
        }
    }
    Ok(())
}

pub fn evidence_usage(command: &str) -> String {
    match command {
        "verify-evidence" => "Usage: bir-rules-codegen verify-evidence \
             --packet DIR [--vault DIR]\n\
             \x20 --packet DIR       packet directory (OS path)\n\
             \x20 --vault DIR        optional upstream evidence vault (OS path)"
            .to_owned(),
        "import-evidence" => "Usage: bir-rules-codegen import-evidence \
             --packet DIR --staging-root DIR [--repo-root DIR]\n\
             \x20 --packet DIR       verified packet directory (OS path)\n\
             \x20 --staging-root DIR brand-new destination; canonical rules are forbidden\n\
             \x20 --repo-root DIR    repository used to identify canonical rules"
            .to_owned(),
        "stage-form" => "Usage: bir-rules-codegen stage-form \
             --form-id ID --staging-root DIR [--packet DIR] [--repo-root DIR]\n\
             \x20 --form-id ID       canonical rules/forms directory name\n\
             \x20 --staging-root DIR brand-new destination; canonical rules are forbidden\n\
             \x20 --packet DIR       optional reviewed packet; emits an external v2 skeleton workspace\n\
             \x20 --repo-root DIR    source repository (auto-discovered by default)"
            .to_owned(),
        _ => "Evidence commands: verify-evidence, import-evidence, stage-form".to_owned(),
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
        .ok_or_else(|| CodegenError::new(format!("command requires {flag} DIR")))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        DERIVED_CLASSIFICATION, DERIVED_MEDIA_TYPE, DerivedEvidenceFile, DerivedEvidenceKind,
        EVIDENCE_PACKET_FORMAT, EVIDENCE_PACKET_MANIFEST, EvidenceAttestation,
        EvidenceAttestationKind, EvidenceCaptureOperatingSystem, EvidenceCaptureProvenance,
        EvidenceObservation, EvidencePacketManifest, EvidenceReview, EvidenceReviewStatus,
        ImportEvidenceOptions, REQUIRED_ATTESTATIONS, RuleSetSourceState, SourceExcerptLocator,
        StageFormOptions, UpstreamEvidenceFile, VerifyEvidenceOptions, approve_exact_external_root,
        canonical_serialize, evidence_usage, import_evidence, packet_digest,
        read_bound_external_file, sha256_hex, stage_form, validate_utc_timestamp, verify_evidence,
        verify_evidence_from_tree, write_fresh_tree,
    };
    use crate::files::read_external_tree;
    use crate::json::CANONICALIZATION_ID;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn upstream_vault_reader_reasserts_exact_approved_root() {
        let root = temporary_directory("vault-open-scope");
        let vault = root.join("vault");
        fs::create_dir(&vault).expect("create test vault");
        let inside = vault.join("inside.bin");
        let outside = root.join("outside.bin");
        fs::write(&inside, b"inside").expect("write inside vault file");
        fs::write(&outside, b"outside").expect("write outside vault file");
        let vault = fs::canonicalize(vault).expect("canonical vault");
        let inside = fs::canonicalize(inside).expect("canonical inside file");
        let outside = fs::canonicalize(outside).expect("canonical outside file");
        let approved_vault =
            approve_exact_external_root(&vault, "test vault", |_| Ok(())).expect("approve vault");

        assert_eq!(
            read_bound_external_file(&inside, &approved_vault, "test vault file")
                .expect("approved vault file"),
            b"inside"
        );
        let error = read_bound_external_file(&outside, &approved_vault, "test vault file")
            .expect_err("out-of-root vault file must fail in the open callback");
        assert!(error.to_string().contains("approved"));

        drop(approved_vault);
        fs::remove_dir_all(root).expect("remove vault scope fixture");
    }

    struct Fixture {
        root: PathBuf,
        packet: PathBuf,
        vault: PathBuf,
        manifest: EvidencePacketManifest,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove evidence fixture");
        }
    }

    fn fixture(status: EvidenceReviewStatus) -> Fixture {
        let root = temporary_directory("evidence");
        let packet = root.join("packet");
        let vault = root.join("vault");
        fs::create_dir_all(packet.join("derived")).expect("create packet payload");
        fs::create_dir_all(vault.join("upstream/forms/form-a")).expect("create vault");

        let derived_bytes = br#"{"field_ids":["alpha","beta"],"metric_count":2}"#.to_vec();
        let upstream_bytes = b"official upstream bytes".to_vec();
        fs::write(packet.join("derived/structure.json"), &derived_bytes)
            .expect("write derived fixture");
        fs::write(
            vault.join("upstream/forms/form-a/source.bin"),
            &upstream_bytes,
        )
        .expect("write upstream fixture");

        let reviewer = (status != EvidenceReviewStatus::Candidate).then(|| "reviewer-1".to_owned());
        let reviewed_at =
            (status != EvidenceReviewStatus::Candidate).then(|| "2026-07-26T00:00:00Z".to_owned());
        let mut manifest = EvidencePacketManifest {
            format: EVIDENCE_PACKET_FORMAT.to_owned(),
            canonicalization: CANONICALIZATION_ID.to_owned(),
            packet_id: "form-a-observation-1".to_owned(),
            form_id: "form-a".to_owned(),
            rule_set_id: "form-a-v1".to_owned(),
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
            },
            created_at_utc: "2026-07-26T00:00:00Z".to_owned(),
            review: EvidenceReview {
                status,
                reviewed_by: reviewer,
                reviewed_at_utc: reviewed_at,
            },
            attestations: REQUIRED_ATTESTATIONS
                .into_iter()
                .map(|kind| EvidenceAttestation {
                    kind,
                    attested: true,
                    attested_by: "collector-1".to_owned(),
                    attested_at_utc: "2026-07-26T00:00:00Z".to_owned(),
                    statement: format!("affirmed {}", kind.as_str()),
                })
                .collect(),
            upstream_evidence: vec![UpstreamEvidenceFile {
                evidence_id: "official-source".to_owned(),
                path: "upstream/forms/form-a/source.bin".to_owned(),
                size_bytes: upstream_bytes.len() as u64,
                sha256: sha256_hex(&upstream_bytes),
            }],
            derived_evidence: vec![DerivedEvidenceFile {
                path: "derived/structure.json".to_owned(),
                kind: DerivedEvidenceKind::StructuredDomInventory,
                observation: EvidenceObservation::Observed,
                source_excerpt: None,
                media_type: DERIVED_MEDIA_TYPE.to_owned(),
                classification: DERIVED_CLASSIFICATION.to_owned(),
                review_status: status,
                source_evidence_ids: vec!["official-source".to_owned()],
                size_bytes: derived_bytes.len() as u64,
                sha256: sha256_hex(&derived_bytes),
            }],
            packet_digest_sha256: String::new(),
        };
        manifest.packet_digest_sha256 = packet_digest(
            &manifest,
            &BTreeMap::from([("derived/structure.json".to_owned(), derived_bytes)]),
        )
        .expect("compute packet digest");
        fs::write(
            packet.join(EVIDENCE_PACKET_MANIFEST),
            canonical_serialize(&manifest, "test packet manifest").expect("serialize manifest"),
        )
        .expect("write packet manifest");

        Fixture {
            root,
            packet,
            vault,
            manifest,
        }
    }

    fn temporary_directory(label: &str) -> PathBuf {
        let path = crate::test_temp_dir().join(format!(
            "bir-rules-codegen-{label}-{}-{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create test root");
        path
    }

    fn rewrite_manifest(fixture: &mut Fixture, mutate: impl FnOnce(&mut EvidencePacketManifest)) {
        mutate(&mut fixture.manifest);
        let derived =
            fs::read(fixture.packet.join("derived/structure.json")).expect("read derived fixture");
        fixture.manifest.packet_digest_sha256 = packet_digest(
            &fixture.manifest,
            &BTreeMap::from([("derived/structure.json".to_owned(), derived)]),
        )
        .expect("compute mutated digest");
        fs::write(
            fixture.packet.join(EVIDENCE_PACKET_MANIFEST),
            canonical_serialize(&fixture.manifest, "test packet manifest")
                .expect("serialize mutated manifest"),
        )
        .expect("rewrite packet manifest");
    }

    fn source_excerpt_locator(fixture: &Fixture) -> SourceExcerptLocator {
        let upstream = &fixture.manifest.upstream_evidence[0];
        SourceExcerptLocator {
            upstream_evidence_id: upstream.evidence_id.clone(),
            full_file_path: upstream.path.clone(),
            full_file_size_bytes: upstream.size_bytes,
            full_file_sha256: upstream.sha256.clone(),
            excerpt_start_byte: 0,
            excerpt_end_byte: 8,
            excerpt_sha256: sha256_hex(b"official"),
        }
    }

    #[test]
    fn vault_is_optional_but_never_reported_as_verified_when_absent() {
        let fixture = fixture(EvidenceReviewStatus::Reviewed);
        let without = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect("verify portable packet");
        assert!(!without.full_upstream_verified);

        let mut with_options = VerifyEvidenceOptions::new(&fixture.packet);
        with_options.vault_dir = Some(fixture.vault.clone());
        let with = verify_evidence(&with_options).expect("verify packet and vault");
        assert!(with.full_upstream_verified);
        assert_eq!(
            with.packet_digest_sha256,
            fixture.manifest.packet_digest_sha256
        );
    }

    #[test]
    fn captured_packet_tree_is_verified_without_reopening_manifest_bytes() {
        let fixture = fixture(EvidenceReviewStatus::Reviewed);
        let captured = read_external_tree(&fixture.packet).expect("capture packet tree");
        fs::write(
            fixture.packet.join(EVIDENCE_PACKET_MANIFEST),
            b"replaced after capture",
        )
        .expect("replace live manifest after capture");

        let report =
            verify_evidence_from_tree(&VerifyEvidenceOptions::new(&fixture.packet), &captured)
                .expect("verify exact captured packet tree");
        assert_eq!(report.packet_id, fixture.manifest.packet_id);
        assert_eq!(
            report.packet_digest_sha256,
            fixture.manifest.packet_digest_sha256
        );
        verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("ordinary verification must observe the replacement");
    }

    #[test]
    fn exact_identity_and_capture_provenance_are_mandatory_and_semantic() {
        let make_fixture = fixture;
        let fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        let mut value =
            serde_json::to_value(&fixture.manifest).expect("serialize manifest to JSON value");
        value
            .as_object_mut()
            .expect("manifest is an object")
            .remove("rule_set_id");
        fs::write(
            fixture.packet.join(EVIDENCE_PACKET_MANIFEST),
            canonical_serialize(&value, "manifest missing rule_set_id")
                .expect("serialize incomplete manifest"),
        )
        .expect("write incomplete manifest");
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("missing exact identity must fail");
        assert!(format!("{error:?}").contains("missing field `rule_set_id`"));

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.official_package_evidence_id = "undeclared-package".to_owned();
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("package evidence identity must resolve");
        assert!(
            error
                .to_string()
                .contains("does not name declared upstream evidence")
        );

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.capture_provenance.tool_commit = "abc123".to_owned();
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("abbreviated tool commit must fail");
        assert!(error.to_string().contains("full 40-character"));

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.capture_provenance.command_argv.clear();
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("capture argv must be present");
        assert!(error.to_string().contains("command_argv"));

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.capture_provenance.command_argv[1] = "unrelated-command".to_owned();
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("capture argv must name the exact source verifier");
        assert!(error.to_string().contains("exact source-map verifier"));

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.source_map_sha256 = "abbreviated".to_owned();
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("manifest source-map digest must be exact");
        assert!(error.to_string().contains("source_map_sha256"));

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.capture_provenance.finished_at_utc = "2026-07-25T23:59:59Z".to_owned();
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("capture finish before start must fail");
        assert!(error.to_string().contains("must not precede"));
    }

    #[test]
    fn utc_timestamps_must_be_real_gregorian_dates() {
        validate_utc_timestamp("2024-02-29T23:59:59Z", "leap timestamp")
            .expect("Gregorian leap day must verify");
        for impossible in [
            "0000-01-01T00:00:00Z",
            "2023-02-29T00:00:00Z",
            "2024-02-30T00:00:00Z",
            "2026-04-31T00:00:00Z",
            "2026-12-00T00:00:00Z",
        ] {
            let error = validate_utc_timestamp(impossible, "test timestamp")
                .expect_err("impossible calendar date must fail");
            assert!(error.to_string().contains("not a real Gregorian"));
        }
    }

    #[test]
    fn tracked_v1_digest_is_required_while_planned_rule_sets_cannot_fabricate_one() {
        let make_fixture = fixture;
        let fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        let value =
            serde_json::to_value(&fixture.manifest).expect("serialize planned source state");
        assert_eq!(
            value["rule_set_source_state"]["source_set_sha256"],
            serde_json::Value::Null
        );

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.tracked_v1_source_set_sha256 = "abbreviated".to_owned();
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("tracked v1 source digest must be real");
        assert!(error.to_string().contains("tracked_v1_source_set_sha256"));

        let fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        let mut value =
            serde_json::to_value(&fixture.manifest).expect("serialize planned source state");
        value["rule_set_source_state"]["source_set_sha256"] =
            serde_json::Value::String("2".repeat(64));
        fs::write(
            fixture.packet.join(EVIDENCE_PACKET_MANIFEST),
            canonical_serialize(&value, "planned state with invented source digest")
                .expect("serialize adversarial planned state"),
        )
        .expect("write adversarial planned state");
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("planned rule set must require null source_set_sha256");
        assert!(format!("{error:?}").contains("invalid type"));

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.rule_set_source_state = RuleSetSourceState::Pinned {
                source_set_sha256: "2".repeat(64),
            };
        });
        verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect("pinned rule set with real source digest must verify");
    }

    #[test]
    fn explicit_not_observed_and_gap_branches_do_not_invent_locators() {
        let make_fixture = fixture;
        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].observation = EvidenceObservation::NotObserved {
                reason: "DOM inventory capture was not run.".to_owned(),
            };
        });
        verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect("explicit not-observed branch must verify");

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].kind = DerivedEvidenceKind::SourceExcerpt;
            manifest.derived_evidence[0].observation = EvidenceObservation::Gap {
                reason: "The upstream source was unavailable to the capture operator.".to_owned(),
            };
        });
        verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect("explicit source-excerpt gap without invented locator must verify");

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].observation = EvidenceObservation::Gap {
                reason: String::new(),
            };
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("reasonless gap must fail");
        assert!(error.to_string().contains("observation reason"));

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        let locator = source_excerpt_locator(&fixture);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].kind = DerivedEvidenceKind::SourceExcerpt;
            manifest.derived_evidence[0].observation = EvidenceObservation::Gap {
                reason: "The source excerpt was unavailable.".to_owned(),
            };
            manifest.derived_evidence[0].source_excerpt = Some(locator);
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("gap branch with invented locator must fail");
        assert!(error.to_string().contains("must not invent a locator"));
    }

    #[test]
    fn observed_source_excerpt_is_bound_to_full_file_and_vault_bytes() {
        let make_fixture = fixture;
        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        let locator = source_excerpt_locator(&fixture);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].kind = DerivedEvidenceKind::SourceExcerpt;
            manifest.derived_evidence[0].source_excerpt = Some(locator);
        });
        let mut options = VerifyEvidenceOptions::new(&fixture.packet);
        options.vault_dir = Some(fixture.vault.clone());
        verify_evidence(&options).expect("exact source excerpt locator and bytes must verify");

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].kind = DerivedEvidenceKind::SourceExcerpt;
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("observed source excerpt without locator must fail");
        assert!(error.to_string().contains("requires a full-file locator"));

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        let mut locator = source_excerpt_locator(&fixture);
        locator.full_file_size_bytes -= 1;
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].kind = DerivedEvidenceKind::SourceExcerpt;
            manifest.derived_evidence[0].source_excerpt = Some(locator);
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("source locator drift must fail");
        assert!(error.to_string().contains("does not exactly match"));

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        let mut locator = source_excerpt_locator(&fixture);
        locator.full_file_path = "C:/official/source.bin".to_owned();
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].kind = DerivedEvidenceKind::SourceExcerpt;
            manifest.derived_evidence[0].source_excerpt = Some(locator);
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("machine-local source locator must fail");
        assert!(error.to_string().contains("non-portable path component"));

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        let locator = source_excerpt_locator(&fixture);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].source_excerpt = Some(locator);
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("non-source kind with excerpt locator must fail");
        assert!(error.to_string().contains("must leave source_excerpt null"));

        let mut fixture = make_fixture(EvidenceReviewStatus::Reviewed);
        let mut locator = source_excerpt_locator(&fixture);
        locator.excerpt_sha256 = "0".repeat(64);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].kind = DerivedEvidenceKind::SourceExcerpt;
            manifest.derived_evidence[0].source_excerpt = Some(locator);
        });
        let mut options = VerifyEvidenceOptions::new(&fixture.packet);
        options.vault_dir = Some(fixture.vault.clone());
        let error = verify_evidence(&options).expect_err("excerpt hash drift must fail");
        assert!(
            error
                .to_string()
                .contains("source excerpt SHA-256 mismatch")
        );
    }

    #[test]
    fn reviewed_import_copies_only_declared_derived_files_and_refuses_overwrite() {
        let fixture = fixture(EvidenceReviewStatus::Reviewed);
        let staging = fixture.root.join("import");
        let report = import_evidence(&ImportEvidenceOptions::new(&fixture.packet, &staging))
            .expect("import reviewed packet");
        assert_eq!(report.imported_file_count, 1);
        assert_eq!(
            fs::read(staging.join("derived/structure.json")).expect("read imported file"),
            fs::read(fixture.packet.join("derived/structure.json")).expect("read packet file")
        );
        assert!(!staging.join(EVIDENCE_PACKET_MANIFEST).exists());

        let error = import_evidence(&ImportEvidenceOptions::new(&fixture.packet, &staging))
            .expect_err("existing staging root must fail");
        assert!(error.to_string().contains("refusing to overwrite"));
    }

    #[test]
    fn candidate_packet_cannot_be_imported() {
        let fixture = fixture(EvidenceReviewStatus::Candidate);
        let staging = fixture.root.join("candidate-import");
        let error = import_evidence(&ImportEvidenceOptions::new(&fixture.packet, &staging))
            .expect_err("candidate import must fail");
        assert!(error.to_string().contains("import requires `reviewed`"));
        assert!(!staging.exists());
    }

    #[test]
    fn noncanonical_duplicate_and_undeclared_packet_content_fails_closed() {
        let fixture = fixture(EvidenceReviewStatus::Reviewed);
        let manifest_path = fixture.packet.join(EVIDENCE_PACKET_MANIFEST);
        let mut noncanonical = fs::read(&manifest_path).expect("read manifest");
        noncanonical.push(b'\n');
        fs::write(&manifest_path, noncanonical).expect("write noncanonical manifest");
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("trailing newline is noncanonical");
        assert!(error.to_string().contains("not canonical"));

        fs::write(&manifest_path, br#"{"format":"a","format":"b"}"#)
            .expect("write duplicate-key manifest");
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("duplicate keys must fail");
        assert!(format!("{error:?}").contains("duplicate object key"));
    }

    #[test]
    fn path_escape_absolute_path_hash_size_and_extra_file_are_rejected() {
        let mut fixture = fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].path = "../escape.json".to_owned();
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("path escape must fail");
        assert!(error.to_string().contains("forbidden path component"));

        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].path = "/absolute.json".to_owned();
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("absolute path must fail");
        assert!(error.to_string().contains("normalized relative path"));

        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].path = r"derived\structure.json".to_owned();
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("backslash path must fail");
        assert!(error.to_string().contains("must use `/`"));

        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].path = "derived/structure.json".to_owned();
            manifest.derived_evidence[0].size_bytes += 1;
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("size mismatch must fail");
        assert!(error.to_string().contains("size mismatch"));

        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].size_bytes -= 1;
            manifest.derived_evidence[0].sha256 = "0".repeat(64);
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("hash mismatch must fail");
        assert!(error.to_string().contains("SHA-256 mismatch"));

        let restored_hash = sha256_hex(
            &fs::read(fixture.packet.join("derived/structure.json")).expect("read derived fixture"),
        );
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.derived_evidence[0].sha256 = restored_hash;
        });
        fs::write(fixture.packet.join("undeclared.json"), b"{}")
            .expect("write undeclared packet file");
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("undeclared content must fail");
        assert!(error.to_string().contains("undeclared=[undeclared.json]"));
    }

    #[test]
    fn sensitive_values_and_online_submission_material_are_rejected() {
        for (document, expected) in [
            (br#"{"password":"not-allowed"}"#.as_slice(), "forbidden"),
            (
                br#"{"note":"taxpayer@example.test"}"#.as_slice(),
                "credential",
            ),
            (br#"{"note":"123-456-789"}"#.as_slice(), "credential"),
            (
                br#"{"note":"https://example.test/submit"}"#.as_slice(),
                "credential",
            ),
            (br#"{"note":123456789}"#.as_slice(), "taxpayer-shaped"),
        ] {
            let mut fixture = fixture(EvidenceReviewStatus::Reviewed);
            fs::write(fixture.packet.join("derived/structure.json"), document)
                .expect("write sensitive document");
            rewrite_manifest(&mut fixture, |manifest| {
                manifest.derived_evidence[0].size_bytes = document.len() as u64;
                manifest.derived_evidence[0].sha256 = sha256_hex(document);
            });
            let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
                .expect_err("sensitive derived content must fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn vault_hash_mismatch_is_not_downgraded_to_partial_success() {
        let fixture = fixture(EvidenceReviewStatus::Reviewed);
        fs::write(
            fixture.vault.join("upstream/forms/form-a/source.bin"),
            b"tampered",
        )
        .expect("tamper vault");
        let mut options = VerifyEvidenceOptions::new(&fixture.packet);
        options.vault_dir = Some(fixture.vault.clone());
        let error = verify_evidence(&options).expect_err("vault mismatch must fail");
        assert!(
            error.to_string().contains("size mismatch")
                || error.to_string().contains("SHA-256 mismatch")
        );
    }

    #[test]
    fn packet_digest_mismatch_is_rejected() {
        let mut fixture = fixture(EvidenceReviewStatus::Reviewed);
        fixture.manifest.packet_digest_sha256 = "0".repeat(64);
        fs::write(
            fixture.packet.join(EVIDENCE_PACKET_MANIFEST),
            canonical_serialize(&fixture.manifest, "test packet manifest")
                .expect("serialize tampered manifest"),
        )
        .expect("write tampered manifest");
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("packet digest mismatch must fail");
        assert!(error.to_string().contains("packet digest mismatch"));
    }

    #[test]
    fn import_refuses_an_explicit_canonical_rules_target() {
        let fixture = fixture(EvidenceReviewStatus::Reviewed);
        let canonical_rules = fixture.root.join("repo/rules");
        fs::create_dir_all(&canonical_rules).expect("create canonical rules marker");
        let target = canonical_rules.join("import");
        let mut options = ImportEvidenceOptions::new(&fixture.packet, &target);
        options.canonical_rules_dir = Some(canonical_rules);
        let error = import_evidence(&options).expect_err("canonical rules import must fail");
        assert!(error.to_string().contains("canonical rules"));
        assert!(!target.exists());
    }

    #[test]
    fn stage_form_mirrors_the_form_and_refuses_canonical_or_existing_targets() {
        let root = temporary_directory("stage-form");
        let repo = root.join("repo");
        let form = repo.join("rules/forms/form-a");
        fs::create_dir_all(&form).expect("create form root");
        fs::create_dir_all(repo.join("crates/bir-rules")).expect("create repository marker");
        fs::write(form.join("manifest.json"), b"{}").expect("write form fixture");

        let staging = root.join("staging");
        let report = stage_form(&StageFormOptions::external_workspace(
            &repo, "form-a", &staging,
        ))
        .expect("stage form");
        assert_eq!(report.staged_file_count, 1);
        assert_eq!(
            fs::read(staging.join("rules/forms/form-a/manifest.json")).expect("read staged form"),
            b"{}"
        );
        let error = stage_form(&StageFormOptions::external_workspace(
            &repo, "form-a", &staging,
        ))
        .expect_err("existing stage must fail");
        assert!(error.to_string().contains("refusing to overwrite"));

        let canonical_target = repo.join("rules/accidental-stage");
        let error = stage_form(&StageFormOptions::external_workspace(
            &repo,
            "form-a",
            &canonical_target,
        ))
        .expect_err("canonical rules target must fail");
        assert!(error.to_string().contains("canonical rules"));
        fs::remove_dir_all(root).expect("remove stage-form fixture");
    }

    #[test]
    fn failed_fresh_tree_write_leaves_partial_target_without_path_cleanup() {
        let root = temporary_directory("fresh-tree-residue");
        let target = root.join("staged");
        let mut files = BTreeMap::new();
        files.insert("a-valid.json".to_owned(), b"{}\n".to_vec());
        files.insert("z/../escape.json".to_owned(), b"{}\n".to_vec());

        let error = write_fresh_tree(&target, &files)
            .expect_err("a non-portable path must abort the fresh write");
        assert!(error.to_string().contains("forbidden path component"));
        assert!(
            target.join("a-valid.json").is_file(),
            "an aborted write leaves its owned partial tree for explicit inspection"
        );
        assert!(error.to_string().contains("left in place"));
        assert!(!root.join("escape.json").exists());
        fs::remove_dir_all(root).expect("remove fresh-tree cleanup fixture");
    }

    #[cfg(unix)]
    #[test]
    fn packet_and_payload_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;

        let fixture = fixture(EvidenceReviewStatus::Reviewed);
        let linked_packet = fixture.root.join("linked-packet");
        symlink(&fixture.packet, &linked_packet).expect("create packet symlink");
        let error = verify_evidence(&VerifyEvidenceOptions::new(&linked_packet))
            .expect_err("packet root symlink must fail");
        assert!(error.to_string().contains("must not be a symlink"));

        fs::remove_file(fixture.packet.join("derived/structure.json"))
            .expect("remove derived fixture");
        symlink(
            fixture.vault.join("upstream/forms/form-a/source.bin"),
            fixture.packet.join("derived/structure.json"),
        )
        .expect("create payload symlink");
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("payload symlink must fail");
        assert!(error.to_string().contains("symlink"));
    }

    #[test]
    fn attestation_set_and_review_metadata_are_closed() {
        let mut fixture = fixture(EvidenceReviewStatus::Reviewed);
        rewrite_manifest(&mut fixture, |manifest| {
            manifest.attestations.pop();
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("missing attestation must fail");
        assert!(error.to_string().contains("exactly 4 attestations"));

        rewrite_manifest(&mut fixture, |manifest| {
            manifest.attestations = REQUIRED_ATTESTATIONS
                .into_iter()
                .map(|kind| EvidenceAttestation {
                    kind,
                    attested: true,
                    attested_by: "collector-1".to_owned(),
                    attested_at_utc: "2026-07-26T00:00:00Z".to_owned(),
                    statement: "affirmed".to_owned(),
                })
                .collect();
            manifest.review.reviewed_by = None;
        });
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("reviewed packet without reviewer must fail");
        assert!(error.to_string().contains("requires reviewed_by"));
    }

    #[test]
    fn enum_names_are_stable_for_schema_and_reports() {
        assert_eq!(
            EvidenceAttestationKind::NoOnlineSubmission.as_str(),
            "no-online-submission"
        );
        assert_eq!(EvidenceReviewStatus::Reviewed.as_str(), "reviewed");
        assert_eq!(
            DerivedEvidenceKind::StructuredDomInventory.as_str(),
            "structured-dom-inventory"
        );
        assert_eq!(
            DerivedEvidenceKind::RuntimeExactErrors.as_str(),
            "runtime-exact-errors"
        );
        assert_eq!(
            DerivedEvidenceKind::RuntimeValidationOrder.as_str(),
            "runtime-validation-order"
        );
        assert_eq!(
            DerivedEvidenceKind::RuntimeSaveReopen.as_str(),
            "runtime-save-reopen"
        );
    }

    #[test]
    fn checked_in_schema_requires_provenance_and_the_closed_kind_surface() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../schemas/validation-rules/evidence-packet-v1.schema.json"
        ))
        .expect("evidence packet schema must be JSON");
        let required = schema["required"]
            .as_array()
            .expect("manifest required list");
        for field in [
            "rule_set_id",
            "form_code",
            "form_revision",
            "official_package_version",
            "official_package_evidence_id",
            "tracked_v1_source_set_sha256",
            "rule_set_source_state",
            "source_map_sha256",
            "source_verification_sha256",
            "capture_provenance",
        ] {
            assert!(
                required.iter().any(|value| value == field),
                "schema must require {field}"
            );
        }
        let kinds: Vec<&str> = schema["$defs"]["derivedEvidenceKind"]["enum"]
            .as_array()
            .expect("derived kind enum")
            .iter()
            .map(|value| value.as_str().expect("kind must be a string"))
            .collect();
        assert_eq!(
            kinds,
            vec![
                "source-excerpt",
                "structured-dom-inventory",
                "xml-inventory",
                "runtime-exact-errors",
                "runtime-validation-order",
                "runtime-save-reopen",
                "record-census",
                "gap-report",
            ]
        );
        assert_eq!(
            schema["$defs"]["captureProvenance"]["additionalProperties"],
            false
        );
        assert_eq!(
            schema["$defs"]["derivedEvidence"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn supporting_schema_registry_resolves_every_reference_offline() {
        const RETRIEVAL_BASE: &str = "https://schema-registry.invalid/bir/validation-rules/";

        fn is_absolute_uri(value: &str) -> bool {
            let Some(colon) = value.find(':') else {
                return false;
            };
            let scheme = &value[..colon];
            !scheme.is_empty()
                && scheme
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_alphabetic())
                && scheme.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
                })
        }

        fn resolve_resource(base: &str, reference: &str, context: &str) -> String {
            if reference.is_empty() {
                return base.to_owned();
            }
            if is_absolute_uri(reference) {
                return reference.to_owned();
            }
            assert!(
                !base.starts_with("urn:"),
                "{context}: relative schema reference `{reference}` cannot resolve against \
                 non-hierarchical base `{base}`"
            );
            let base = base.split_once('#').map_or(base, |(resource, _)| resource);
            let directory_end = base
                .rfind('/')
                .expect("hierarchical schema retrieval URI must contain `/`");
            format!("{}{reference}", &base[..=directory_end])
        }

        fn assert_refs_resolve(
            value: &serde_json::Value,
            inherited_base: &str,
            registry: &BTreeMap<String, &serde_json::Value>,
            context: &str,
        ) {
            match value {
                serde_json::Value::Object(object) => {
                    let base = object
                        .get("$id")
                        .and_then(serde_json::Value::as_str)
                        .map_or_else(
                            || inherited_base.to_owned(),
                            |id| resolve_resource(inherited_base, id, context),
                        );
                    if let Some(reference) = object.get("$ref").and_then(serde_json::Value::as_str)
                    {
                        let (resource_reference, fragment) =
                            reference.split_once('#').unwrap_or((reference, ""));
                        let resource = resolve_resource(&base, resource_reference, context);
                        let target = registry.get(&resource).unwrap_or_else(|| {
                            panic!(
                                "{context}: schema reference `{reference}` resolved to \
                                 unregistered resource `{resource}`"
                            )
                        });
                        if !fragment.is_empty() {
                            assert!(
                                fragment.starts_with('/') && !fragment.contains('%'),
                                "{context}: schema reference `{reference}` must use a plain \
                                 JSON Pointer fragment"
                            );
                            assert!(
                                target.pointer(fragment).is_some(),
                                "{context}: schema reference `{reference}` points to missing \
                                 fragment `#{fragment}` in `{resource}`"
                            );
                        }
                    }
                    for child in object.values() {
                        assert_refs_resolve(child, &base, registry, context);
                    }
                }
                serde_json::Value::Array(array) => {
                    for child in array {
                        assert_refs_resolve(child, inherited_base, registry, context);
                    }
                }
                _ => {}
            }
        }

        let schema_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/validation-rules");
        let mut documents = BTreeMap::<String, serde_json::Value>::new();
        for entry in fs::read_dir(&schema_root).expect("read supporting schema directory") {
            let entry = entry.expect("read supporting schema entry");
            let name = entry
                .file_name()
                .into_string()
                .expect("supporting schema filename must be UTF-8");
            if !name.ends_with(".schema.json") {
                continue;
            }
            let source = fs::read_to_string(entry.path())
                .unwrap_or_else(|error| panic!("{name}: read schema: {error}"));
            let schema = serde_json::from_str(&source)
                .unwrap_or_else(|error| panic!("{name}: invalid JSON schema: {error}"));
            assert!(
                documents.insert(name.clone(), schema).is_none(),
                "{name}: duplicate supporting schema filename"
            );
        }
        assert!(
            !documents.is_empty(),
            "supporting schema directory must contain schemas"
        );

        let mut registry = BTreeMap::<String, &serde_json::Value>::new();
        for (name, schema) in &documents {
            assert_eq!(
                schema.get("$schema").and_then(serde_json::Value::as_str),
                Some("https://json-schema.org/draft/2020-12/schema"),
                "{name}: schema dialect must stay explicit"
            );
            let retrieval_uri = format!("{RETRIEVAL_BASE}{name}");
            assert!(
                registry.insert(retrieval_uri, schema).is_none(),
                "{name}: duplicate retrieval URI"
            );
            if let Some(id) = schema.get("$id").and_then(serde_json::Value::as_str) {
                assert!(is_absolute_uri(id), "{name}: `$id` must be absolute");
                assert!(
                    registry.insert(id.to_owned(), schema).is_none(),
                    "{name}: duplicate schema `$id` `{id}`"
                );
            }
        }

        for (name, schema) in &documents {
            let retrieval_uri = format!("{RETRIEVAL_BASE}{name}");
            assert_refs_resolve(schema, &retrieval_uri, &registry, name);
        }
    }

    #[test]
    fn unknown_derived_kind_is_not_an_opaque_json_escape_hatch() {
        let fixture = fixture(EvidenceReviewStatus::Reviewed);
        let mut value =
            serde_json::to_value(&fixture.manifest).expect("serialize manifest to JSON value");
        value["derived_evidence"][0]["kind"] = serde_json::json!("opaque-observation");
        fs::write(
            fixture.packet.join(EVIDENCE_PACKET_MANIFEST),
            canonical_serialize(&value, "manifest with unknown evidence kind")
                .expect("serialize adversarial manifest"),
        )
        .expect("write adversarial manifest");
        let error = verify_evidence(&VerifyEvidenceOptions::new(&fixture.packet))
            .expect_err("unknown derived evidence kind must fail");
        assert!(format!("{error:?}").contains("unknown variant"));
    }

    #[test]
    fn command_help_documents_the_closed_interfaces() {
        let verify = evidence_usage("verify-evidence");
        assert!(verify.contains("--packet DIR [--vault DIR]"));
        let import = evidence_usage("import-evidence");
        assert!(import.contains("--packet DIR --staging-root DIR"));
        assert!(import.contains("canonical rules are forbidden"));
        let stage = evidence_usage("stage-form");
        assert!(stage.contains("--form-id ID --staging-root DIR"));
    }

    #[allow(dead_code)]
    fn assert_path(_: &Path) {}
}
