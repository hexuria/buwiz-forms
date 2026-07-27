//! Deterministic, candidate-only scaffolding for the external evidence review ledger.
//!
//! This module deliberately stops before packet review. It binds the exact
//! tracked v1 corpus, its reconciled censuses, the audited v2 identity (currently
//! only 2550Q), and an explicit content-addressed vault catalog into the current
//! `bir-evidence-review-ledger-v1` shape. It never supplies a reviewer, review
//! timestamp, packet-review digest, clock value, user identity, host identity,
//! or capture fact on the caller's behalf.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use same_file::Handle;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audit::{AuditOptions, AuditReport, audit};
use crate::corpus::{CorpusReport, FormResult, ValidateV1Options, validate_v1};
use crate::error::{CodegenError, Result};
use crate::evidence::{
    EvidenceAttestation, EvidenceAttestationKind, EvidenceCaptureProvenance, EvidenceReview,
    EvidenceReviewStatus, RuleSetSourceState,
};
use crate::evidence_set::{EVIDENCE_REVIEW_LEDGER_FORMAT, TRACKED_V1_SOURCE_SET_DOMAIN};
use crate::files::{
    ApprovedExternalFile, read_external_bytes_bound, read_tracked_bytes, read_tracked_tree,
};
use crate::form_integration::PROTECTED_2550Q_RULE_SET_ID;
use crate::hash::{digest_entries, sha256_hex};
use crate::json::{CANONICALIZATION_ID, canonical_bytes, parse_strict};
use crate::model::{BranchState, IndexDocument, ReviewStatus};
use crate::path::{
    canonical_repo_root, is_same_or_below, is_symlink_or_reparse_point, resolve_existing_under,
    validate_portable_relative,
};
use crate::sensitive::reject_sensitive_text;
use crate::vault_acquisition::{
    EVIDENCE_VAULT_CATALOG_FORMAT, EvidenceVaultCatalog, EvidenceVaultCatalogEntry,
    VaultAssetDisposition, validate_source_verifier_provenance, vault_asset_disposition,
};
#[cfg(windows)]
use crate::verified_file::stable_windows_link_count;

pub const EXPECTED_REVIEW_LEDGER_FORM_COUNT: usize = 43;
pub const EVIDENCE_REVIEW_SCAFFOLD_REQUEST_FORMAT: &str = "bir-evidence-review-scaffold-request-v1";

const GAPS_PATH: &str = "derived/gaps.json";
const SUMMARY_PATH: &str = "derived/tracked-v1-summary.json";
const SOURCE_EXCERPT_PREFIX: &str = "derived/source-excerpts/";
const CONTENT_ADDRESS_PREFIX: &str = "upstream/sha256/";
const PROTECTED_2550Q_FORM_ID: &str = "2550q-v2024";

const EXPECTED_TOTAL_JSON_FILES: usize = 659;
const EXPECTED_V1_JSON_FILES: usize = 520;
const EXPECTED_V2_JSON_FILES: usize = 139;
const EXPECTED_V1_FIELDS: usize = 9_592;
const EXPECTED_V1_VALIDATIONS: usize = 2_007;
const EXPECTED_V1_CALCULATIONS: usize = 623;
const EXPECTED_V1_NEGATIVE_FIXTURES: usize = 1_354;
const EXPECTED_V1_SCHEMA_DOCUMENTS: usize = 216;

/// Complete inputs for one candidate ledger entry.
///
/// Every value with human or capture provenance comes from this structure.
/// `review` and the packet-review digest are intentionally not configurable:
/// the emitted ledger always uses candidate/null values.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateEvidenceReviewInput {
    pub form_id: String,
    pub packet_id: String,
    /// Exact manifest asset selected as the official package for this form.
    ///
    /// This is explicit because 1701Q also records a historical package
    /// executable; choosing the first executable would not be evidence-safe.
    pub official_package_asset_id: String,
    pub capture_session_id: String,
    pub source_map_sha256: String,
    pub source_verification_sha256: String,
    pub capture_provenance: EvidenceCaptureProvenance,
    pub created_at_utc: String,
    pub attestations: Vec<EvidenceAttestation>,
    pub source_excerpts: Vec<CandidateSourceExcerpt>,
    pub capture_gaps: Vec<CandidateCaptureGap>,
}

/// Canonical external request consumed by the CLI-facing scaffold.
///
/// This file is deliberately separate from the emitted ledger. It carries
/// caller-supplied capture facts but has no review-status or packet-digest
/// fields that could be used to self-approve the output.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReviewScaffoldRequest {
    pub format: String,
    pub canonicalization: String,
    pub entries: Vec<CandidateEvidenceReviewInput>,
}

/// Caller-reviewed locator metadata. No upstream bytes enter the ledger.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSourceExcerpt {
    pub excerpt_id: String,
    pub upstream_evidence_id: String,
    pub excerpt_start_byte: u64,
    pub excerpt_end_byte: u64,
    pub excerpt_sha256: String,
}

/// Caller-supplied explicit evidence gap.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateCaptureGap {
    pub gap_id: String,
    pub reason: String,
    pub source_evidence_ids: Vec<String>,
}

/// Inputs for the exact 43-form candidate ledger.
#[derive(Clone, Debug)]
pub struct ScaffoldEvidenceReviewLedgerOptions {
    pub repo_root: PathBuf,
    pub vault_catalog: PathBuf,
    /// Fresh `.json` file beneath an existing real directory outside the repo.
    pub output_path: PathBuf,
    /// Must be an exact ordered bijection with `rules/index.json`.
    pub entries: Vec<CandidateEvidenceReviewInput>,
    pub dry_run: bool,
}

impl ScaffoldEvidenceReviewLedgerOptions {
    pub fn new(
        repo_root: impl Into<PathBuf>,
        vault_catalog: impl Into<PathBuf>,
        output_path: impl Into<PathBuf>,
        entries: Vec<CandidateEvidenceReviewInput>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            vault_catalog: vault_catalog.into(),
            output_path: output_path.into(),
            entries,
            dry_run: false,
        }
    }
}

/// One immutable binding returned for human inspection alongside the bytes.
#[derive(Clone, Debug, Serialize)]
pub struct ScaffoldedFormBinding {
    pub ordinal: usize,
    pub form_id: String,
    pub rule_set_id: String,
    pub tracked_v1_source_set_sha256: String,
    pub official_package_asset_id: String,
    pub official_package_evidence_id: String,
    pub fields: usize,
    pub validations: usize,
    pub calculations: usize,
    pub negative_fixtures: usize,
}

/// Exact plan/result. Dry-run and write mode return the same JSON bytes/digest.
#[derive(Clone, Debug, Serialize)]
pub struct ScaffoldEvidenceReviewLedgerReport {
    pub entry_count: usize,
    pub ledger_sha256: String,
    pub rules_index_sha256: String,
    pub vault_catalog_sha256: String,
    pub output_path: PathBuf,
    pub written: bool,
    #[serde(skip)]
    pub canonical_json_bytes: Vec<u8>,
    pub bindings: Vec<ScaffoldedFormBinding>,
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

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug)]
struct ManifestAsset {
    asset_id: String,
    kind: String,
    sha256: String,
    size_bytes: u64,
    disposition: VaultAssetDisposition,
}

#[derive(Clone, Debug)]
struct FormPlan {
    index: RulesIndexEntry,
    tracked_v1_source_set_sha256: String,
    rule_set_id: String,
    rule_set_source_state: RuleSetSourceState,
    assets: Vec<ManifestAsset>,
    official_package_asset: ManifestAsset,
    upstream_evidence_ids: BTreeSet<String>,
    census: FormResult,
}

#[derive(Clone, Debug)]
struct TrackedSource {
    path: String,
    canonical_bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct CatalogIndex {
    by_tuple: BTreeMap<(String, u64), EvidenceVaultCatalogEntry>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewLedger {
    format: String,
    canonicalization: String,
    entries: Vec<ReviewLedgerEntry>,
}

#[derive(Clone, Debug, Serialize)]
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
    source_excerpts: Vec<CandidateSourceExcerpt>,
    capture_gaps: Vec<CandidateCaptureGap>,
    expected_packet_digest_sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct DerivedReview {
    path: String,
    status: EvidenceReviewStatus,
}

/// Plans or writes a fresh, external, candidate-only evidence review ledger.
pub fn scaffold_evidence_review_ledger(
    options: &ScaffoldEvidenceReviewLedgerOptions,
) -> Result<ScaffoldEvidenceReviewLedgerReport> {
    let repo_root = canonical_repo_root_strict(&options.repo_root)?;
    let output_path = validate_fresh_external_output(&repo_root, &options.output_path)?;
    let catalog_file = open_verified_external_scaffold_input(
        &options.vault_catalog,
        "evidence vault catalog",
        |_| Ok(()),
    )?;
    let catalog_bytes = read_external_bytes_bound(catalog_file, "evidence vault catalog")?;
    let catalog =
        load_canonical_typed::<EvidenceVaultCatalog>(&catalog_bytes, "evidence vault catalog")?;
    let catalog_index = validate_catalog(&catalog)?;

    let index_path = resolve_existing_under(&repo_root, "rules/index.json", "rules index")?;
    let index_bytes = read_tracked_bytes(&index_path)?;
    let index_json = parse_strict(&index_bytes, &index_path)?;
    let rules_index_sha256 = sha256_hex(&canonical_bytes(&index_json));
    let index: RulesIndex = serde_json::from_value(index_json.into_serde()).map_err(|source| {
        CodegenError::with_source("load closed rules/index.json structure", source)
    })?;
    validate_rules_index(&index)?;
    validate_input_bijection(&index.forms, &options.entries)?;

    let corpus = validate_v1(&ValidateV1Options::new(&repo_root))?;
    validate_production_census(&corpus)?;
    let census_by_form = index_census(&corpus)?;

    let v2_audit = audit(&AuditOptions::tracked_checkout(&repo_root))?;
    let protected_v2 = load_protected_v2_identity(&repo_root, &v2_audit, &index)?;

    let mut expected_catalog_tuples = BTreeSet::<(String, u64)>::new();
    let mut plans = Vec::with_capacity(EXPECTED_REVIEW_LEDGER_FORM_COUNT);
    for (index_entry, input) in index.forms.iter().zip(&options.entries) {
        let form_root = resolve_existing_under(
            &repo_root,
            &format!("rules/forms/{}", index_entry.form_id),
            "tracked v1 form root",
        )?;
        let manifest_path = resolve_existing_under(
            &repo_root,
            &format!("rules/{}", index_entry.path),
            "tracked v1 form manifest",
        )?;
        let manifest_value =
            parse_strict(&read_tracked_bytes(&manifest_path)?, &manifest_path)?.into_serde();
        validate_manifest_identity(&manifest_value, index_entry)?;
        let assets = manifest_assets(&manifest_value)?;
        for asset in &assets {
            if asset.disposition == VaultAssetDisposition::Acquirable {
                expected_catalog_tuples.insert((asset.sha256.clone(), asset.size_bytes));
            }
        }
        let official_package_asset = select_official_package_asset(
            &assets,
            &input.official_package_asset_id,
            &index_entry.package_version,
        )?;

        let sources = tracked_v1_sources(&form_root)?;
        let tracked_v1_source_set_sha256 = digest_entries(
            TRACKED_V1_SOURCE_SET_DOMAIN,
            sources
                .iter()
                .map(|source| (source.path.as_str(), source.canonical_bytes.as_slice())),
        );

        let (rule_set_id, rule_set_source_state) = if index_entry.form_id == PROTECTED_2550Q_FORM_ID
        {
            protected_v2.clone()
        } else {
            (
                planned_rule_set_id(index_entry)?,
                RuleSetSourceState::Planned {
                    source_set_sha256: (),
                },
            )
        };

        let census = census_by_form
            .get(&index_entry.form_id)
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "validated v1 census has no exact result for `{}`",
                    index_entry.form_id
                ))
            })?
            .clone();
        plans.push(FormPlan {
            index: index_entry.clone(),
            tracked_v1_source_set_sha256,
            rule_set_id,
            rule_set_source_state,
            assets,
            official_package_asset,
            upstream_evidence_ids: BTreeSet::new(),
            census,
        });
    }

    require_exact_catalog_bijection(&expected_catalog_tuples, &catalog_index)?;
    populate_form_upstream_ids(&catalog_index, &mut plans)?;
    verify_tracked_source_bindings(&repo_root, &plans)?;

    let (ledger, bindings) = build_candidate_ledger(&plans, &options.entries, &catalog_index)?;
    let canonical_json_bytes = canonical_serialize(&ledger, "candidate evidence review ledger")?;
    reject_sensitive_json(
        &serde_json::to_value(&ledger).map_err(|source| {
            CodegenError::with_source("serialize candidate ledger for safety audit", source)
        })?,
        "candidate evidence review ledger",
    )?;
    if canonical_json_bytes.contains(&b'\r') {
        return Err(CodegenError::new(
            "canonical candidate ledger unexpectedly contains a carriage return",
        ));
    }
    std::str::from_utf8(&canonical_json_bytes).map_err(|source| {
        CodegenError::with_source("canonical candidate ledger is not UTF-8", source)
    })?;
    let ledger_sha256 = sha256_hex(&canonical_json_bytes);

    let written = install_candidate_ledger(&output_path, &canonical_json_bytes, options.dry_run)?;

    Ok(ScaffoldEvidenceReviewLedgerReport {
        entry_count: ledger.entries.len(),
        ledger_sha256,
        rules_index_sha256,
        vault_catalog_sha256: sha256_hex(&catalog_bytes),
        output_path,
        written,
        canonical_json_bytes,
        bindings,
    })
}

/// Load one exact canonical, external scaffold request.
pub fn load_evidence_review_scaffold_request(
    repo_root: &Path,
    input_path: &Path,
) -> Result<EvidenceReviewScaffoldRequest> {
    let repo_root = canonical_repo_root_strict(repo_root)?;
    let input_file = open_verified_external_scaffold_input(
        input_path,
        "evidence review scaffold request",
        |resolved| {
            if is_same_or_below(&repo_root, resolved) {
                return Err(CodegenError::new(
                    "evidence review scaffold request must remain outside the repository",
                ));
            }
            Ok(())
        },
    )?;
    let bytes = read_external_bytes_bound(input_file, "evidence review scaffold request")?;
    let parsed = parse_strict(&bytes, Path::new("evidence review scaffold request"))?;
    if canonical_bytes(&parsed) != bytes {
        return Err(CodegenError::new(
            "evidence review scaffold request is not exact canonical JSON",
        ));
    }
    let serde_value = parsed.into_serde();
    reject_sensitive_json(&serde_value, "evidence review scaffold request")?;
    let request: EvidenceReviewScaffoldRequest =
        serde_json::from_value(serde_value).map_err(|source| {
            CodegenError::with_source(
                "load closed evidence review scaffold request structure",
                source,
            )
        })?;
    if request.format != EVIDENCE_REVIEW_SCAFFOLD_REQUEST_FORMAT
        || request.canonicalization != CANONICALIZATION_ID
    {
        return Err(CodegenError::new(format!(
            "evidence review scaffold request must use format `{EVIDENCE_REVIEW_SCAFFOLD_REQUEST_FORMAT}` and canonicalization `{CANONICALIZATION_ID}`"
        )));
    }
    Ok(request)
}

fn validate_production_census(report: &CorpusReport) -> Result<()> {
    let observed = (
        report.forms_audited,
        report.total_json_files,
        report.json_files,
        report.v2_json_files,
        report.fields,
        report.validations,
        report.calculations,
        report.negative_fixtures,
        report.schema_documents,
    );
    let expected = (
        EXPECTED_REVIEW_LEDGER_FORM_COUNT,
        EXPECTED_TOTAL_JSON_FILES,
        EXPECTED_V1_JSON_FILES,
        EXPECTED_V2_JSON_FILES,
        EXPECTED_V1_FIELDS,
        EXPECTED_V1_VALIDATIONS,
        EXPECTED_V1_CALCULATIONS,
        EXPECTED_V1_NEGATIVE_FIXTURES,
        EXPECTED_V1_SCHEMA_DOCUMENTS,
    );
    if observed != expected {
        return Err(CodegenError::new(format!(
            "tracked corpus census drift blocks evidence-review scaffolding: expected forms/total-json/v1-json/v2-json/fields/validations/calculations/negative/schema={expected:?}, observed={observed:?}"
        )));
    }
    Ok(())
}

fn index_census(report: &CorpusReport) -> Result<BTreeMap<String, FormResult>> {
    let mut by_form = BTreeMap::new();
    for form in &report.form_results {
        if by_form.insert(form.form_id.clone(), form.clone()).is_some() {
            return Err(CodegenError::new(format!(
                "validated v1 census contains duplicate form_id `{}`",
                form.form_id
            )));
        }
    }
    if by_form.len() != EXPECTED_REVIEW_LEDGER_FORM_COUNT {
        return Err(CodegenError::new(
            "validated v1 per-form census is not an exact 43-form set",
        ));
    }
    Ok(by_form)
}

fn validate_rules_index(index: &RulesIndex) -> Result<()> {
    if index.schema != "./schema/form-manifest.schema.json"
        || index.schema_version != "1.0.0"
        || index.knowledge_base != "offline-ebirforms-validation-rules"
    {
        return Err(CodegenError::new(
            "rules/index.json metadata does not match the closed v1 corpus contract",
        ));
    }
    validate_iso_date(&index.updated, "rules index updated")?;
    if index.forms.len() != EXPECTED_REVIEW_LEDGER_FORM_COUNT
        || index.priority_queue.len() != EXPECTED_REVIEW_LEDGER_FORM_COUNT
    {
        return Err(CodegenError::new(format!(
            "review ledger requires exactly {EXPECTED_REVIEW_LEDGER_FORM_COUNT} rules/index.json forms and priority entries"
        )));
    }

    let mut form_ids = BTreeSet::new();
    let mut exact_identities = BTreeSet::new();
    for (offset, entry) in index.forms.iter().enumerate() {
        validate_portable_identifier(&entry.form_id, "rules index form_id")?;
        validate_exact_identity(&entry.form_code, "rules index form_code")?;
        validate_iso_date(&entry.revision, "rules index revision")?;
        validate_exact_identity(&entry.package_version, "rules index package_version")?;
        if entry.status != "complete" {
            return Err(CodegenError::new(format!(
                "rules index form `{}` must remain v1 status `complete`",
                entry.form_id
            )));
        }
        if entry.priority != offset + 1 {
            return Err(CodegenError::new(
                "rules/index.json forms must be in exact contiguous priority order",
            ));
        }
        if index.priority_queue[offset] != entry.form_code {
            return Err(CodegenError::new(format!(
                "rules/index.json priority_queue mismatch at ordinal {}",
                offset + 1
            )));
        }
        let expected_path = format!("forms/{}/manifest.json", entry.form_id);
        if entry.path != expected_path {
            return Err(CodegenError::new(format!(
                "rules index path for `{}` must be exact `{expected_path}`",
                entry.form_id
            )));
        }
        validate_portable_relative(&entry.path, "rules index manifest path")?;
        if !form_ids.insert(entry.form_id.as_str()) {
            return Err(CodegenError::new(format!(
                "duplicate rules/index form_id `{}`",
                entry.form_id
            )));
        }
        if !exact_identities.insert((
            entry.form_code.as_str(),
            entry.revision.as_str(),
            entry.package_version.as_str(),
        )) {
            return Err(CodegenError::new(
                "rules/index.json contains a duplicate exact form identity",
            ));
        }
    }
    Ok(())
}

fn validate_input_bijection(
    forms: &[RulesIndexEntry],
    inputs: &[CandidateEvidenceReviewInput],
) -> Result<()> {
    let expected: Vec<&str> = forms.iter().map(|entry| entry.form_id.as_str()).collect();
    let actual: Vec<&str> = inputs.iter().map(|entry| entry.form_id.as_str()).collect();
    if expected != actual {
        return Err(CodegenError::new(format!(
            "candidate capture inputs must be an exact ordered bijection with rules/index.json; expected=[{}] actual=[{}]",
            expected.join(", "),
            actual.join(", ")
        )));
    }
    let mut packet_ids = BTreeSet::new();
    for input in inputs {
        validate_portable_identifier(&input.form_id, "candidate input form_id")?;
        validate_portable_identifier(&input.packet_id, "candidate input packet_id")?;
        validate_portable_identifier(
            &input.official_package_asset_id,
            "candidate input official_package_asset_id",
        )?;
        validate_portable_identifier(
            &input.capture_session_id,
            "candidate input capture_session_id",
        )?;
        validate_sha256(
            &input.source_map_sha256,
            "candidate input source_map_sha256",
        )?;
        validate_sha256(
            &input.source_verification_sha256,
            "candidate input source_verification_sha256",
        )?;
        if !packet_ids.insert(input.packet_id.as_str()) {
            return Err(CodegenError::new(format!(
                "candidate inputs contain duplicate packet_id `{}`",
                input.packet_id
            )));
        }
        validate_source_verifier_provenance(&input.capture_provenance)?;
        validate_utc_timestamp(&input.created_at_utc, "candidate created_at_utc")?;
        validate_explicit_attestations(&input.attestations)?;
        validate_source_excerpt_inputs(&input.source_excerpts)?;
        validate_capture_gap_inputs(&input.capture_gaps)?;
    }
    Ok(())
}

fn load_protected_v2_identity(
    repo_root: &Path,
    audit_report: &AuditReport,
    rules_index: &RulesIndex,
) -> Result<(String, RuleSetSourceState)> {
    if audit_report.snapshot_count() != 1 {
        return Err(CodegenError::new(format!(
            "candidate ledger scaffold requires exactly one audited v2 snapshot (protected 2550Q); found {}",
            audit_report.snapshot_count()
        )));
    }
    let v2_index_path =
        resolve_existing_under(repo_root, "rules/ir/v2/index.json", "v2 rules index")?;
    let bytes = read_tracked_bytes(&v2_index_path)?;
    let strict = parse_strict(&bytes, &v2_index_path)?;
    let index: IndexDocument = serde_json::from_value(strict.into_serde()).map_err(|source| {
        CodegenError::with_source("load closed v2 rules index structure", source)
    })?;
    if index.schema != "../../schema/v2/index.schema.json"
        || index.schema_version != "2.0.0"
        || index.snapshots.len() != 1
    {
        return Err(CodegenError::new(
            "v2 index must contain only the exact protected 2550Q snapshot",
        ));
    }
    let snapshot = &index.snapshots[0];
    let protected_form = rules_index
        .forms
        .iter()
        .find(|entry| entry.form_id == PROTECTED_2550Q_FORM_ID)
        .ok_or_else(|| CodegenError::new("rules index is missing protected 2550Q form identity"))?;
    if snapshot.rule_set_id != PROTECTED_2550Q_RULE_SET_ID
        || snapshot.form_code != protected_form.form_code
        || snapshot.form_revision != protected_form.revision
        || snapshot.official_package_version != protected_form.package_version
        || snapshot.path != format!("{PROTECTED_2550Q_RULE_SET_ID}/rule-set.json")
        || snapshot.review_status != ReviewStatus::Candidate
        || snapshot.profile_states.official != BranchState::Executable
        || snapshot.profile_states.filing_safe != BranchState::Unresolved
    {
        return Err(CodegenError::new(
            "protected 2550Q v2 identity, candidate status, or profile boundary drifted",
        ));
    }
    let source_set_sha256 = snapshot.source_set_sha256.as_deref().ok_or_else(|| {
        CodegenError::new("protected 2550Q v2 snapshot must carry its exact pinned source digest")
    })?;
    validate_sha256(source_set_sha256, "protected 2550Q source_set_sha256")?;

    let audited = audit_report.require_rule_set(PROTECTED_2550Q_RULE_SET_ID)?;
    if audited.rule_set_id() != snapshot.rule_set_id.as_str()
        || audited.form_code() != snapshot.form_code.as_str()
        || audited.form_revision() != snapshot.form_revision.as_str()
        || audited.official_package_version() != snapshot.official_package_version.as_str()
        || audited.source_set_sha256() != Some(source_set_sha256)
        || audited.review_status() != "candidate"
    {
        return Err(CodegenError::new(
            "protected 2550Q index identity does not match the fully audited snapshot",
        ));
    }

    Ok((
        snapshot.rule_set_id.clone(),
        RuleSetSourceState::Pinned {
            source_set_sha256: source_set_sha256.to_owned(),
        },
    ))
}

fn planned_rule_set_id(index: &RulesIndexEntry) -> Result<String> {
    let rule_set_id = format!("{}-p{}", index.form_id, index.package_version);
    validate_portable_identifier(&rule_set_id, "planned rule_set_id")?;
    Ok(rule_set_id)
}

fn validate_manifest_identity(manifest: &Value, index: &RulesIndexEntry) -> Result<()> {
    for (key, expected) in [
        ("form_id", index.form_id.as_str()),
        ("form_code", index.form_code.as_str()),
        ("revision", index.revision.as_str()),
        ("package_version", index.package_version.as_str()),
        ("status", index.status.as_str()),
    ] {
        let actual = manifest
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| CodegenError::new(format!("form manifest is missing string `{key}`")))?;
        if actual != expected {
            return Err(CodegenError::new(format!(
                "form manifest `{key}` mismatch for `{}`: index={expected} manifest={actual}",
                index.form_id
            )));
        }
    }
    Ok(())
}

fn manifest_assets(manifest: &Value) -> Result<Vec<ManifestAsset>> {
    let assets = manifest
        .get("official_assets")
        .and_then(Value::as_array)
        .ok_or_else(|| CodegenError::new("form manifest official_assets must be an array"))?;
    if assets.is_empty() {
        return Err(CodegenError::new(
            "form manifest official_assets must not be empty",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut result = Vec::with_capacity(assets.len());
    for (offset, value) in assets.iter().enumerate() {
        let object = value.as_object().ok_or_else(|| {
            CodegenError::new(format!("official_assets[{offset}] must be an object"))
        })?;
        // Machine-local `path`, prose `notes`, and taxpayer-shaped payload
        // locations are intentionally never projected into this ledger.
        let asset_id = required_string(object, "asset_id", "official asset")?.to_owned();
        let kind = required_string(object, "kind", "official asset")?.to_owned();
        let sha256 = required_string(object, "sha256", "official asset")?.to_owned();
        let size_bytes = object.get("size").and_then(Value::as_u64).ok_or_else(|| {
            CodegenError::new(format!("official_assets[{offset}].size is not an integer"))
        })?;
        validate_portable_identifier(&asset_id, "official asset_id")?;
        validate_portable_identifier(&kind, "official asset kind")?;
        validate_sha256(&sha256, "official asset sha256")?;
        if !ids.insert(asset_id.clone()) {
            return Err(CodegenError::new(format!(
                "form manifest contains duplicate official asset_id `{asset_id}`"
            )));
        }
        let disposition = vault_asset_disposition(&kind, size_bytes).map_err(|source| {
            CodegenError::with_source(
                format!("official asset `{asset_id}` has no safe vault disposition"),
                source,
            )
        })?;
        result.push(ManifestAsset {
            asset_id,
            kind,
            sha256,
            size_bytes,
            disposition,
        });
    }
    Ok(result)
}

fn select_official_package_asset(
    assets: &[ManifestAsset],
    explicit_asset_id: &str,
    package_version: &str,
) -> Result<ManifestAsset> {
    let expected_asset_id = official_package_asset_id(package_version)?;
    if explicit_asset_id != expected_asset_id.as_str() {
        return Err(CodegenError::new(format!(
            "explicit official_package_asset_id `{explicit_asset_id}` does not bind exact package version `{package_version}`; expected `{expected_asset_id}`"
        )));
    }
    let matches: Vec<&ManifestAsset> = assets
        .iter()
        .filter(|asset| asset.asset_id == explicit_asset_id)
        .collect();
    let [asset] = matches.as_slice() else {
        return Err(CodegenError::new(format!(
            "explicit official_package_asset_id `{explicit_asset_id}` must select exactly one manifest asset; found {}",
            matches.len()
        )));
    };
    if asset.kind != "official-package-executable"
        || asset.size_bytes == 0
        || asset.disposition != VaultAssetDisposition::Acquirable
    {
        return Err(CodegenError::new(format!(
            "explicit official package asset `{explicit_asset_id}` must be a non-empty acquirable official-package-executable"
        )));
    }
    Ok((*asset).clone())
}

fn official_package_asset_id(package_version: &str) -> Result<String> {
    let components: Vec<&str> = package_version.split('.').collect();
    if components.len() != 4
        || components[3] != "0"
        || components.iter().any(|component| {
            component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return Err(CodegenError::new(format!(
            "official package version `{package_version}` has no reviewed exact manifest asset-id projection"
        )));
    }
    let asset_id = format!(
        "package-{}.{}.{}",
        components[0], components[1], components[2]
    );
    validate_portable_identifier(&asset_id, "official package asset_id projection")?;
    Ok(asset_id)
}

fn validate_catalog(catalog: &EvidenceVaultCatalog) -> Result<CatalogIndex> {
    if catalog.format != EVIDENCE_VAULT_CATALOG_FORMAT
        || catalog.canonicalization != CANONICALIZATION_ID
    {
        return Err(CodegenError::new(format!(
            "vault catalog must use `{EVIDENCE_VAULT_CATALOG_FORMAT}` and `{CANONICALIZATION_ID}`"
        )));
    }
    reject_sensitive_json(
        &serde_json::to_value(catalog).map_err(|source| {
            CodegenError::with_source("serialize vault catalog for safety audit", source)
        })?,
        "vault catalog",
    )?;
    if catalog.entries.is_empty() {
        return Err(CodegenError::new("vault catalog entries must not be empty"));
    }
    let mut by_tuple = BTreeMap::new();
    let mut evidence_ids = BTreeSet::new();
    let mut catalog_capture_binding: Option<(String, String, String, String)> = None;
    let mut previous: Option<&str> = None;
    for entry in &catalog.entries {
        validate_portable_identifier(&entry.evidence_id, "catalog evidence_id")?;
        validate_sha256(&entry.sha256, "catalog sha256")?;
        validate_sha256(&entry.source_map_sha256, "catalog source_map_sha256")?;
        validate_sha256(
            &entry.source_verification_sha256,
            "catalog source_verification_sha256",
        )?;
        validate_portable_identifier(&entry.capture_session_id, "catalog capture_session_id")?;
        if entry.size_bytes == 0 {
            return Err(CodegenError::new(format!(
                "catalog evidence `{}` must not claim zero-byte content",
                entry.evidence_id
            )));
        }
        if previous.is_some_and(|value| value >= entry.evidence_id.as_str()) {
            return Err(CodegenError::new(
                "vault catalog entries must be strictly ordered by evidence_id",
            ));
        }
        previous = Some(&entry.evidence_id);
        let expected_id = format!("sha256-{}", entry.sha256);
        if entry.evidence_id != expected_id {
            return Err(CodegenError::new(format!(
                "catalog evidence_id `{}` must be exact content identity `{expected_id}`",
                entry.evidence_id
            )));
        }
        let expected_path = content_addressed_path(&entry.sha256);
        if entry.content_path != expected_path {
            return Err(CodegenError::new(format!(
                "catalog evidence `{}` content_path must be `{expected_path}`",
                entry.evidence_id
            )));
        }
        validate_source_verifier_provenance(&entry.capture_provenance)?;
        reject_sensitive_json(
            &serde_json::to_value(&entry.capture_provenance).map_err(|source| {
                CodegenError::with_source("serialize catalog capture provenance", source)
            })?,
            "catalog capture provenance",
        )?;
        let provenance_digest = sha256_hex(&canonical_serialize(
            &entry.capture_provenance,
            "catalog capture provenance",
        )?);
        let capture_binding = (
            entry.capture_session_id.clone(),
            entry.source_map_sha256.clone(),
            entry.source_verification_sha256.clone(),
            provenance_digest,
        );
        if catalog_capture_binding
            .as_ref()
            .is_some_and(|expected| expected != &capture_binding)
        {
            return Err(CodegenError::new(format!(
                "catalog entries must share one capture session and exact source verification/provenance binding; `{}` differs",
                entry.capture_session_id
            )));
        }
        catalog_capture_binding = Some(capture_binding);
        let tuple = (entry.sha256.clone(), entry.size_bytes);
        if by_tuple.insert(tuple, entry.clone()).is_some() {
            return Err(CodegenError::new(
                "vault catalog contains duplicate content hash/size identity",
            ));
        }
        if !evidence_ids.insert(entry.evidence_id.clone()) {
            return Err(CodegenError::new(format!(
                "vault catalog contains duplicate evidence_id `{}`",
                entry.evidence_id
            )));
        }
    }
    Ok(CatalogIndex { by_tuple })
}

fn require_exact_catalog_bijection(
    expected: &BTreeSet<(String, u64)>,
    catalog: &CatalogIndex,
) -> Result<()> {
    let actual: BTreeSet<(String, u64)> = catalog.by_tuple.keys().cloned().collect();
    let missing: Vec<String> = expected
        .difference(&actual)
        .map(|(sha256, size)| format!("{sha256}/{size}"))
        .collect();
    let extra: Vec<String> = actual
        .difference(expected)
        .map(|(sha256, size)| format!("{sha256}/{size}"))
        .collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(CodegenError::new(format!(
            "vault catalog must be an exact content-identity bijection with all acquirable tracked manifest assets; missing=[{}] extra=[{}]",
            missing.join(", "),
            extra.join(", ")
        )));
    }
    Ok(())
}

fn populate_form_upstream_ids(catalog: &CatalogIndex, plans: &mut [FormPlan]) -> Result<()> {
    for plan in plans {
        for asset in plan
            .assets
            .iter()
            .filter(|asset| asset.disposition == VaultAssetDisposition::Acquirable)
        {
            let entry = catalog
                .by_tuple
                .get(&(asset.sha256.clone(), asset.size_bytes))
                .ok_or_else(|| {
                    CodegenError::new(format!(
                        "catalog identity vanished for `{}` asset `{}`",
                        plan.index.form_id, asset.asset_id
                    ))
                })?;
            plan.upstream_evidence_ids.insert(entry.evidence_id.clone());
        }
        if plan.upstream_evidence_ids.is_empty() {
            return Err(CodegenError::new(format!(
                "form `{}` has no acquirable content-addressed upstream identity",
                plan.index.form_id
            )));
        }
    }
    Ok(())
}

fn verify_tracked_source_bindings(repo_root: &Path, plans: &[FormPlan]) -> Result<()> {
    for plan in plans {
        let form_root = resolve_existing_under(
            repo_root,
            &format!("rules/forms/{}", plan.index.form_id),
            "tracked v1 form root",
        )?;
        let sources = tracked_v1_sources(&form_root)?;
        let observed = digest_entries(
            TRACKED_V1_SOURCE_SET_DOMAIN,
            sources
                .iter()
                .map(|source| (source.path.as_str(), source.canonical_bytes.as_slice())),
        );
        if observed != plan.tracked_v1_source_set_sha256 {
            return Err(CodegenError::new(format!(
                "tracked v1 sources for `{}` changed during ledger planning",
                plan.index.form_id
            )));
        }
    }
    Ok(())
}

fn build_candidate_ledger(
    plans: &[FormPlan],
    inputs: &[CandidateEvidenceReviewInput],
    catalog: &CatalogIndex,
) -> Result<(ReviewLedger, Vec<ScaffoldedFormBinding>)> {
    let mut entries = Vec::with_capacity(plans.len());
    let mut bindings = Vec::with_capacity(plans.len());
    for (offset, (plan, input)) in plans.iter().zip(inputs).enumerate() {
        if plan.index.form_id != input.form_id {
            return Err(CodegenError::new(
                "internal form/input order drift while building candidate ledger",
            ));
        }
        let official_entry = catalog
            .by_tuple
            .get(&(
                plan.official_package_asset.sha256.clone(),
                plan.official_package_asset.size_bytes,
            ))
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "official package tuple for `{}` has no exact catalog evidence identity",
                    plan.index.form_id
                ))
            })?
            .evidence_id
            .clone();
        if !plan.upstream_evidence_ids.contains(&official_entry) {
            return Err(CodegenError::new(format!(
                "official package evidence `{official_entry}` is not selected for form `{}`",
                plan.index.form_id
            )));
        }

        validate_input_sources(input, &plan.upstream_evidence_ids, catalog)?;
        validate_selected_capture_attribution(input, &plan.upstream_evidence_ids, catalog)?;

        let mut derived_paths = BTreeSet::from([GAPS_PATH.to_owned(), SUMMARY_PATH.to_owned()]);
        for excerpt in &input.source_excerpts {
            derived_paths.insert(source_excerpt_path(&excerpt.excerpt_id)?);
        }
        let derived_reviews = derived_paths
            .into_iter()
            .map(|path| DerivedReview {
                path,
                status: EvidenceReviewStatus::Candidate,
            })
            .collect();
        entries.push(ReviewLedgerEntry {
            form_id: input.form_id.clone(),
            packet_id: input.packet_id.clone(),
            rule_set_id: plan.rule_set_id.clone(),
            tracked_v1_source_set_sha256: plan.tracked_v1_source_set_sha256.clone(),
            rule_set_source_state: plan.rule_set_source_state.clone(),
            official_package_asset_id: input.official_package_asset_id.clone(),
            capture_session_id: input.capture_session_id.clone(),
            source_map_sha256: input.source_map_sha256.clone(),
            source_verification_sha256: input.source_verification_sha256.clone(),
            capture_provenance: input.capture_provenance.clone(),
            created_at_utc: input.created_at_utc.clone(),
            review: EvidenceReview {
                status: EvidenceReviewStatus::Candidate,
                reviewed_by: None,
                reviewed_at_utc: None,
            },
            attestations: input.attestations.clone(),
            derived_reviews,
            source_excerpts: input.source_excerpts.clone(),
            capture_gaps: input.capture_gaps.clone(),
            // This is the current schema's packet-review digest field.
            // Candidate scaffolding can never populate it.
            expected_packet_digest_sha256: None,
        });
        bindings.push(ScaffoldedFormBinding {
            ordinal: offset + 1,
            form_id: plan.index.form_id.clone(),
            rule_set_id: plan.rule_set_id.clone(),
            tracked_v1_source_set_sha256: plan.tracked_v1_source_set_sha256.clone(),
            official_package_asset_id: plan.official_package_asset.asset_id.clone(),
            official_package_evidence_id: official_entry,
            fields: plan.census.fields,
            validations: plan.census.validations,
            calculations: plan.census.calculations,
            negative_fixtures: plan.census.negative_fixtures,
        });
    }
    if entries.len() != EXPECTED_REVIEW_LEDGER_FORM_COUNT {
        return Err(CodegenError::new(
            "candidate review ledger construction did not produce exactly 43 entries",
        ));
    }
    Ok((
        ReviewLedger {
            format: EVIDENCE_REVIEW_LEDGER_FORMAT.to_owned(),
            canonicalization: CANONICALIZATION_ID.to_owned(),
            entries,
        },
        bindings,
    ))
}

fn validate_input_sources(
    input: &CandidateEvidenceReviewInput,
    allowed_evidence_ids: &BTreeSet<String>,
    catalog: &CatalogIndex,
) -> Result<()> {
    for excerpt in &input.source_excerpts {
        if !allowed_evidence_ids.contains(&excerpt.upstream_evidence_id) {
            return Err(CodegenError::new(format!(
                "source excerpt `{}` cites catalog identity `{}` not selected by form `{}`",
                excerpt.excerpt_id, excerpt.upstream_evidence_id, input.form_id
            )));
        }
        let upstream = catalog
            .by_tuple
            .values()
            .find(|entry| entry.evidence_id.as_str() == excerpt.upstream_evidence_id.as_str())
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "source excerpt `{}` cites missing catalog evidence identity `{}`",
                    excerpt.excerpt_id, excerpt.upstream_evidence_id
                ))
            })?;
        if excerpt.excerpt_end_byte > upstream.size_bytes {
            return Err(CodegenError::new(format!(
                "source excerpt `{}` byte range exceeds catalog evidence `{}` size {}",
                excerpt.excerpt_id, excerpt.upstream_evidence_id, upstream.size_bytes
            )));
        }
    }
    for gap in &input.capture_gaps {
        for evidence_id in &gap.source_evidence_ids {
            if !allowed_evidence_ids.contains(evidence_id) {
                return Err(CodegenError::new(format!(
                    "capture gap `{}` cites catalog identity `{evidence_id}` not selected by form `{}`",
                    gap.gap_id, input.form_id
                )));
            }
        }
    }
    Ok(())
}

fn validate_selected_capture_attribution(
    input: &CandidateEvidenceReviewInput,
    selected_evidence_ids: &BTreeSet<String>,
    catalog: &CatalogIndex,
) -> Result<()> {
    let input_provenance =
        canonical_serialize(&input.capture_provenance, "candidate capture provenance")?;
    let mut matched = 0_usize;
    for entry in catalog
        .by_tuple
        .values()
        .filter(|entry| selected_evidence_ids.contains(&entry.evidence_id))
    {
        matched += 1;
        if entry.capture_session_id != input.capture_session_id {
            return Err(CodegenError::new(format!(
                "candidate form `{}` capture_session_id `{}` must exactly match selected catalog evidence `{}` capture_session_id `{}`",
                input.form_id,
                input.capture_session_id,
                entry.evidence_id,
                entry.capture_session_id
            )));
        }
        if entry.source_map_sha256 != input.source_map_sha256
            || entry.source_verification_sha256 != input.source_verification_sha256
        {
            return Err(CodegenError::new(format!(
                "candidate form `{}` source-map and verifier digests must exactly match selected catalog evidence `{}`",
                input.form_id, entry.evidence_id
            )));
        }
        let catalog_provenance = canonical_serialize(
            &entry.capture_provenance,
            "selected catalog capture provenance",
        )?;
        if catalog_provenance != input_provenance {
            return Err(CodegenError::new(format!(
                "candidate form `{}` capture_provenance must exactly match selected catalog evidence `{}` capture_provenance",
                input.form_id, entry.evidence_id
            )));
        }
    }
    if matched != selected_evidence_ids.len() {
        return Err(CodegenError::new(format!(
            "candidate form `{}` selected upstream evidence identities are not an exact catalog subset",
            input.form_id
        )));
    }
    Ok(())
}

fn validate_source_excerpt_inputs(excerpts: &[CandidateSourceExcerpt]) -> Result<()> {
    let mut previous: Option<&str> = None;
    for excerpt in excerpts {
        validate_portable_identifier(&excerpt.excerpt_id, "source excerpt_id")?;
        validate_portable_identifier(
            &excerpt.upstream_evidence_id,
            "source excerpt upstream_evidence_id",
        )?;
        validate_sha256(&excerpt.excerpt_sha256, "source excerpt sha256")?;
        if excerpt.excerpt_start_byte >= excerpt.excerpt_end_byte {
            return Err(CodegenError::new(format!(
                "source excerpt `{}` byte range must be non-empty",
                excerpt.excerpt_id
            )));
        }
        if previous.is_some_and(|value| value >= excerpt.excerpt_id.as_str()) {
            return Err(CodegenError::new(
                "source excerpts must be strictly ordered by excerpt_id",
            ));
        }
        previous = Some(&excerpt.excerpt_id);
    }
    Ok(())
}

fn validate_capture_gap_inputs(gaps: &[CandidateCaptureGap]) -> Result<()> {
    let mut previous: Option<&str> = None;
    for gap in gaps {
        validate_portable_identifier(&gap.gap_id, "capture gap_id")?;
        validate_safe_human_text(&gap.reason, "capture gap reason")?;
        if previous.is_some_and(|value| value >= gap.gap_id.as_str()) {
            return Err(CodegenError::new(
                "capture gaps must be strictly ordered by gap_id",
            ));
        }
        previous = Some(&gap.gap_id);
        if gap.source_evidence_ids.is_empty() {
            return Err(CodegenError::new(format!(
                "capture gap `{}` must cite at least one upstream evidence id",
                gap.gap_id
            )));
        }
        let mut previous_source: Option<&str> = None;
        for source in &gap.source_evidence_ids {
            validate_portable_identifier(source, "capture gap source_evidence_id")?;
            if previous_source.is_some_and(|value| value >= source.as_str()) {
                return Err(CodegenError::new(format!(
                    "capture gap `{}` source_evidence_ids must be strictly ordered",
                    gap.gap_id
                )));
            }
            previous_source = Some(source);
        }
    }
    Ok(())
}

fn tracked_v1_sources(form_root: &Path) -> Result<Vec<TrackedSource>> {
    let tree = read_tracked_tree(form_root)?;
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
        sources.push(TrackedSource {
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

fn content_addressed_path(sha256: &str) -> String {
    format!("{CONTENT_ADDRESS_PREFIX}{}/{}", &sha256[..2], sha256)
}

fn source_excerpt_path(excerpt_id: &str) -> Result<String> {
    validate_portable_identifier(excerpt_id, "source excerpt_id")?;
    Ok(format!("{SOURCE_EXCERPT_PREFIX}{excerpt_id}.json"))
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| CodegenError::new(format!("{label} is missing string `{key}`")))
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

fn validate_exact_identity(value: &str, label: &str) -> Result<()> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > 256
        || value.chars().any(char::is_control)
    {
        return Err(CodegenError::new(format!(
            "{label} must be exact, non-empty, trimmed, control-free text"
        )));
    }
    reject_unsafe_text(value, label)
}

fn validate_safe_human_text(value: &str, label: &str) -> Result<()> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > 1_024
        || value.chars().any(char::is_control)
    {
        return Err(CodegenError::new(format!(
            "{label} must be non-empty, trimmed, bounded, control-free text"
        )));
    }
    reject_unsafe_text(value, label)
}

fn validate_explicit_attestations(attestations: &[EvidenceAttestation]) -> Result<()> {
    let expected = [
        EvidenceAttestationKind::DerivedOnly,
        EvidenceAttestationKind::NoTaxpayerValues,
        EvidenceAttestationKind::NoCredentials,
        EvidenceAttestationKind::NoOnlineSubmission,
    ];
    if attestations.len() != expected.len() {
        return Err(CodegenError::new(
            "candidate ledger input must explicitly contain exactly four attestations",
        ));
    }
    for (attestation, expected_kind) in attestations.iter().zip(expected) {
        if attestation.kind != expected_kind || !attestation.attested {
            return Err(CodegenError::new(
                "candidate ledger attestations must be explicitly affirmed in contract order",
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

fn validate_iso_date(value: &str, label: &str) -> Result<()> {
    validate_utc_timestamp(&format!("{value}T00:00:00Z"), label)
}

fn reject_unsafe_text(value: &str, label: &str) -> Result<()> {
    if looks_like_machine_locator(value) {
        return Err(CodegenError::new(format!(
            "{label} contains machine-local material"
        )));
    }
    reject_sensitive_text(value, label)
}

fn reject_sensitive_json(value: &Value, label: &str) -> Result<()> {
    match value {
        Value::Array(values) => {
            for value in values {
                reject_sensitive_json(value, label)?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                let normalized: String = key
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect();
                if forbidden_sensitive_key(&normalized) {
                    return Err(CodegenError::new(format!(
                        "{label} contains forbidden sensitive/transport key `{key}`"
                    )));
                }
                reject_sensitive_json(value, label)?;
            }
        }
        Value::String(value) => reject_unsafe_text(value, label)?,
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn forbidden_sensitive_key(normalized: &str) -> bool {
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

fn looks_like_machine_locator(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let bytes = value.as_bytes();
    let drive_path = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    drive_path
        || value.starts_with("\\\\")
        || value.starts_with("//")
        || lower.starts_with("file:")
        || lower.starts_with("/users/")
        || lower.starts_with("/volumes/")
        || lower.starts_with("/home/")
        || lower.starts_with("/root/")
        || lower.starts_with("/tmp/")
        || lower.starts_with("/var/")
        || lower.starts_with("/etc/")
        || lower.starts_with("~/")
        || lower.starts_with("~\\")
        || lower.contains("%userprofile%")
        || lower.contains("${home}")
        || lower.contains("\\users\\")
}

fn canonical_repo_root_strict(path: &Path) -> Result<PathBuf> {
    let absolute = absolute_normalized(path)?;
    reject_symlink_ancestors(&absolute, "repository root")?;
    let metadata = fs::symlink_metadata(&absolute)
        .map_err(|source| CodegenError::io("inspect repository root", &absolute, source))?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "repository root `{}` must be a real directory",
            absolute.display()
        )));
    }
    let root = canonical_repo_root(&absolute)?;
    let _ = resolve_existing_under(&root, "rules/index.json", "rules index")?;
    let _ = resolve_existing_under(&root, "crates/bir-rules", "repository marker")?;
    Ok(root)
}

fn open_verified_external_scaffold_input<F>(
    path: &Path,
    label: &str,
    validate_resolved_path: F,
) -> Result<ApprovedExternalFile>
where
    F: Fn(&Path) -> Result<()>,
{
    // Inspect the caller's lexical path before normalization so a forbidden
    // component cannot be hidden behind `..` and then echoed by a path error.
    reject_sensitive_scaffold_input_path(path, label)?;
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(CodegenError::new(format!(
            "{label} must be lexically normalized"
        )));
    }
    let absolute = absolute_normalized(path)?;
    reject_sensitive_scaffold_input_path(&absolute, label)?;
    let expected = fs::canonicalize(&absolute)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label}"), &absolute, source))?;
    ApprovedExternalFile::capture(&absolute, label, |resolved| {
        // The verified-file helper invokes this callback before opening and
        // again after opening while it proves handle/path identity. Keeping
        // the closed sensitive-root policy inside the callback prevents a
        // replacement race from redirecting either external input.
        if resolved != expected {
            return Err(CodegenError::new(format!(
                "{label} resolved to a different exact canonical file"
            )));
        }
        reject_sensitive_scaffold_input_path(resolved, label)?;
        validate_resolved_path(resolved)
    })
}

fn reject_sensitive_scaffold_input_path(path: &Path, label: &str) -> Result<()> {
    let components: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_ascii_lowercase()),
            _ => None,
        })
        .collect();
    let has_pair = |left: &str, right: &str| {
        components
            .windows(2)
            .any(|pair| pair[0] == left && pair[1] == right)
    };
    let sensitive = has_pair("ebirforms", "savefile")
        || has_pair("ebirforms", "profile")
        || components
            .iter()
            .any(|component| component == "group.dev.goldcoders.bir")
        || components
            .last()
            .is_some_and(|component| component == "bir_data.db")
        || components.iter().any(|component| {
            matches!(
                component.as_str(),
                "taxpayer-data"
                    | "taxpayer_data"
                    | "live-taxpayer-data"
                    | ".ssh"
                    | ".aws"
                    | ".azure"
                    | ".gnupg"
                    | ".kube"
                    | ".docker"
                    | "keychain"
                    | "keychains"
                    | "credential"
                    | "credentials"
                    | "secrets"
            )
        });
    if sensitive {
        return Err(CodegenError::new(format!(
            "{label} is beneath a forbidden credential/taxpayer/save/live-database root"
        )));
    }
    Ok(())
}

fn validate_fresh_external_output(repo_root: &Path, path: &Path) -> Result<PathBuf> {
    let absolute = absolute_normalized(path)?;
    if absolute.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err(CodegenError::new(format!(
            "candidate ledger output `{}` must have a `.json` extension",
            absolute.display()
        )));
    }
    if absolute.exists() {
        return Err(CodegenError::new(format!(
            "candidate ledger output `{}` already exists; refusing to overwrite",
            absolute.display()
        )));
    }
    reject_symlink_ancestors(&absolute, "candidate ledger output")?;
    let parent = absolute.parent().ok_or_else(|| {
        CodegenError::new(format!(
            "candidate ledger output `{}` has no parent",
            absolute.display()
        ))
    })?;
    let metadata = fs::symlink_metadata(parent).map_err(|source| {
        CodegenError::io("inspect candidate ledger output parent", parent, source)
    })?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "candidate ledger output parent `{}` must be an existing real directory",
            parent.display()
        )));
    }
    let canonical_parent = fs::canonicalize(parent).map_err(|source| {
        CodegenError::io(
            "canonicalize candidate ledger output parent",
            parent,
            source,
        )
    })?;
    if is_same_or_below(repo_root, &absolute) || is_same_or_below(repo_root, &canonical_parent) {
        return Err(CodegenError::new(format!(
            "candidate ledger output `{}` must remain outside repository `{}`",
            absolute.display(),
            repo_root.display()
        )));
    }
    let file_name = absolute.file_name().ok_or_else(|| {
        CodegenError::new("candidate ledger output must name a regular JSON file")
    })?;
    let file_name_text = file_name
        .to_str()
        .ok_or_else(|| CodegenError::new("candidate ledger output name must be valid UTF-8"))?;
    if file_name_text.is_empty()
        || file_name_text.chars().any(char::is_control)
        || file_name_text.contains(':')
        || file_name_text.contains('\\')
    {
        return Err(CodegenError::new(
            "candidate ledger output name must be a portable UTF-8 file name",
        ));
    }
    Ok(canonical_parent.join(file_name))
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

fn reject_symlink_ancestors(path: &Path, label: &str) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if is_symlink_or_reparse_point(&metadata) => {
                return Err(CodegenError::new(format!(
                    "{label} `{}` traverses symlink/reparse point `{}`",
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

fn load_canonical_typed<T>(bytes: &[u8], label: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let value = parse_strict(&bytes, Path::new(label))?;
    if bytes != canonical_bytes(&value) {
        return Err(CodegenError::new(format!(
            "{label} is not canonical `{CANONICALIZATION_ID}` JSON"
        )));
    }
    let typed = serde_json::from_value(value.into_serde()).map_err(|source| {
        CodegenError::with_source(format!("closed-structure load of {label} failed"), source)
    })?;
    Ok(typed)
}

fn canonical_serialize(value: &impl Serialize, label: &str) -> Result<Vec<u8>> {
    let ordinary = serde_json::to_vec(value)
        .map_err(|source| CodegenError::with_source(format!("serialize {label}"), source))?;
    let parsed = parse_strict(&ordinary, Path::new(label))?;
    Ok(canonical_bytes(&parsed))
}

fn write_fresh_file(target: &Path, bytes: &[u8]) -> Result<()> {
    write_fresh_file_with_hook(target, bytes, |_| Ok(()))
}

fn write_fresh_file_with_hook<F>(target: &Path, bytes: &[u8], after_create: F) -> Result<()>
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    match fs::symlink_metadata(target) {
        Ok(_) => {
            return Err(CodegenError::new(format!(
                "candidate ledger output `{}` already exists; refusing to overwrite",
                target.display()
            )));
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(CodegenError::io(
                "inspect candidate ledger output before create",
                target,
                source,
            ));
        }
    }
    reject_symlink_ancestors(target, "candidate ledger output")?;
    let parent = target.parent().ok_or_else(|| {
        CodegenError::new(format!(
            "candidate ledger output `{}` has no parent",
            target.display()
        ))
    })?;
    let parent_handle = Handle::from_path(parent).map_err(|source| {
        CodegenError::io(
            "identify candidate ledger output parent before create",
            parent,
            source,
        )
    })?;
    let mut output_file = match OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(target)
    {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(CodegenError::new(format!(
                "candidate ledger output `{}` appeared before create; refusing to overwrite",
                target.display()
            )));
        }
        Err(source) => {
            return Err(CodegenError::io(
                "create fresh candidate ledger output",
                target,
                source,
            ));
        }
    };
    let opened_handle = (|| {
        let cloned = output_file.try_clone().map_err(|source| {
            CodegenError::io("clone fresh candidate ledger output handle", target, source)
        })?;
        Handle::from_file(cloned).map_err(|source| {
            CodegenError::io(
                "identify fresh candidate ledger output handle",
                target,
                source,
            )
        })
    })()
    .map_err(|source| incomplete_fresh_output_error(target, "candidate ledger", source))?;

    // The final name is created directly, so a failed write can leave a
    // visible incomplete file. That file is intentionally never removed by
    // pathname: a same-user process may have substituted the name meanwhile.
    let operation = (|| {
        after_create(target).map_err(|source| {
            CodegenError::io(
                "run candidate ledger post-create verification hook",
                target,
                source,
            )
        })?;
        verify_fresh_output_identity(
            target,
            parent,
            &parent_handle,
            &opened_handle,
            &output_file,
            "candidate ledger output",
        )?;
        output_file.write_all(bytes).map_err(|source| {
            CodegenError::io("write fresh candidate ledger output", target, source)
        })?;
        output_file.sync_all().map_err(|source| {
            CodegenError::io("sync fresh candidate ledger output", target, source)
        })?;
        output_file.seek(SeekFrom::Start(0)).map_err(|source| {
            CodegenError::io("rewind fresh candidate ledger output", target, source)
        })?;
        let mut installed_bytes = Vec::new();
        output_file
            .read_to_end(&mut installed_bytes)
            .map_err(|source| {
                CodegenError::io("read fresh candidate ledger output handle", target, source)
            })?;
        if installed_bytes != bytes {
            return Err(CodegenError::new(
                "fresh candidate ledger bytes drifted after write",
            ));
        }
        verify_fresh_output_identity(
            target,
            parent,
            &parent_handle,
            &opened_handle,
            &output_file,
            "candidate ledger output",
        )?;
        sync_directory(parent)?;
        verify_fresh_output_identity(
            target,
            parent,
            &parent_handle,
            &opened_handle,
            &output_file,
            "candidate ledger output",
        )?;
        Ok(())
    })();
    operation.map_err(|source| incomplete_fresh_output_error(target, "candidate ledger", source))
}

fn incomplete_fresh_output_error(target: &Path, label: &str, source: CodegenError) -> CodegenError {
    CodegenError::with_source(
        format!(
            "fresh {label} output `{}` may be incomplete and was deliberately left in place; no path cleanup was attempted: {source}",
            target.display(),
        ),
        source,
    )
}

// This repeated handle/path/parent comparison narrows same-user races but
// cannot make path lookup transactional: an actor with write access to the
// parent can still replace entries after the final check. The safety property
// here is fail-closed non-destruction, not immunity from that actor.
fn verify_fresh_output_identity(
    target: &Path,
    parent: &Path,
    parent_handle: &Handle,
    opened_handle: &Handle,
    opened_file: &File,
    label: &str,
) -> Result<()> {
    reject_symlink_ancestors(target, label)?;
    let current_parent = Handle::from_path(parent).map_err(|source| {
        CodegenError::io(&format!("reidentify {label} parent"), parent, source)
    })?;
    if &current_parent != parent_handle {
        return Err(CodegenError::new(format!(
            "{label} parent `{}` was replaced during fresh output construction",
            parent.display()
        )));
    }
    let path_metadata = fs::symlink_metadata(target)
        .map_err(|source| CodegenError::io(&format!("inspect current {label}"), target, source))?;
    if is_symlink_or_reparse_point(&path_metadata) || !path_metadata.is_file() {
        return Err(CodegenError::new(format!(
            "{label} `{}` changed to a non-regular or symlink/reparse entry",
            target.display()
        )));
    }
    let current_handle = Handle::from_path(target).map_err(|source| {
        CodegenError::io(&format!("reidentify current {label}"), target, source)
    })?;
    if &current_handle != opened_handle {
        return Err(CodegenError::new(format!(
            "{label} `{}` was substituted after create_new",
            target.display()
        )));
    }
    let opened_metadata = opened_file.metadata().map_err(|source| {
        CodegenError::io(&format!("inspect opened {label} handle"), target, source)
    })?;
    if is_symlink_or_reparse_point(&opened_metadata) || !opened_metadata.is_file() {
        return Err(CodegenError::new(format!(
            "opened {label} handle for `{}` is not a real regular file",
            target.display()
        )));
    }
    reject_hard_link_alias(opened_file, &opened_metadata, target, label)
}

#[cfg(unix)]
fn reject_hard_link_alias(
    _file: &File,
    metadata: &Metadata,
    path: &Path,
    label: &str,
) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    if metadata.nlink() != 1 {
        return Err(CodegenError::new(format!(
            "{label} `{}` has {} hard links; aliased fresh outputs are forbidden",
            path.display(),
            metadata.nlink()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn reject_hard_link_alias(
    file: &File,
    _metadata: &Metadata,
    path: &Path,
    label: &str,
) -> Result<()> {
    let link_count = stable_windows_link_count(file, path, label)?;
    if link_count != 1 {
        return Err(CodegenError::new(format!(
            "{label} `{path}` has {link_count} hard links; aliased fresh outputs are forbidden",
            path = path.display()
        )));
    }
    Ok(())
}

fn install_candidate_ledger(target: &Path, bytes: &[u8], dry_run: bool) -> Result<bool> {
    if dry_run {
        match fs::symlink_metadata(target) {
            Ok(_) => {
                return Err(CodegenError::new(format!(
                    "candidate ledger output `{}` appeared during dry-run planning; refusing a stale plan",
                    target.display()
                )));
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(CodegenError::io(
                    "inspect candidate ledger output during dry-run",
                    target,
                    source,
                ));
            }
        }
        return Ok(false);
    }
    write_fresh_file(target, bytes)?;
    Ok(true)
}

fn sync_directory(path: &Path) -> Result<()> {
    match File::open(path).and_then(|directory| directory.sync_all()) {
        Ok(()) => Ok(()),
        Err(source) if cfg!(windows) => {
            let _ = source;
            Ok(())
        }
        Err(source) => Err(CodegenError::io(
            "sync candidate ledger output directory",
            path,
            source,
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;

    use serde_json::Value;

    use super::{
        CONTENT_ADDRESS_PREFIX, CandidateCaptureGap, CandidateEvidenceReviewInput,
        CandidateSourceExcerpt, CatalogIndex, DerivedReview, EvidenceReviewStatus, ReviewLedger,
        ReviewLedgerEntry, RuleSetSourceState, VaultAssetDisposition, absolute_normalized,
        canonical_serialize, canonical_text, content_addressed_path, install_candidate_ledger,
        load_evidence_review_scaffold_request, open_verified_external_scaffold_input,
        reject_sensitive_scaffold_input_path, reject_unsafe_text, require_exact_catalog_bijection,
        select_official_package_asset, validate_capture_gap_inputs, validate_fresh_external_output,
        validate_input_bijection, validate_selected_capture_attribution,
        validate_source_excerpt_inputs, write_fresh_file, write_fresh_file_with_hook,
    };
    use crate::evidence::{
        EvidenceAttestation, EvidenceAttestationKind, EvidenceCaptureOperatingSystem,
        EvidenceCaptureProvenance, EvidenceReview,
    };
    use crate::files::read_external_bytes_bound;
    use crate::hash::sha256_hex;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestRoot {
        path: PathBuf,
    }

    impl TestRoot {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "bir-evidence-review-scaffold-{label}-{}-{}",
                std::process::id(),
                TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create test root");
            Self { path }
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            if self.path.exists() {
                fs::remove_dir_all(&self.path).expect("remove owned test root");
            }
        }
    }

    fn provenance() -> EvidenceCaptureProvenance {
        EvidenceCaptureProvenance {
            tool_commit: "a".repeat(40),
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
            attested_by: "review-team".to_owned(),
            attested_at_utc: "2026-07-26T01:00:00Z".to_owned(),
            statement: format!("Explicit value-free attestation for {kind:?}"),
        })
        .collect()
    }

    fn input(form_id: &str) -> CandidateEvidenceReviewInput {
        CandidateEvidenceReviewInput {
            form_id: form_id.to_owned(),
            packet_id: format!("{form_id}-packet"),
            official_package_asset_id: "package-7.9.6".to_owned(),
            capture_session_id: format!("{form_id}-capture"),
            source_map_sha256: "c".repeat(64),
            source_verification_sha256: "d".repeat(64),
            capture_provenance: provenance(),
            created_at_utc: "2026-07-26T01:00:00Z".to_owned(),
            attestations: attestations(),
            source_excerpts: Vec::new(),
            capture_gaps: vec![CandidateCaptureGap {
                gap_id: "source-excerpt-not-captured".to_owned(),
                reason: "No source excerpt has yet been selected for candidate review.".to_owned(),
                source_evidence_ids: vec![format!("sha256-{}", "b".repeat(64))],
            }],
        }
    }

    fn catalog_entry(
        sha_byte: char,
        size_bytes: u64,
        capture_session_id: &str,
        capture_provenance: EvidenceCaptureProvenance,
    ) -> crate::vault_acquisition::EvidenceVaultCatalogEntry {
        let sha256 = sha_byte.to_string().repeat(64);
        crate::vault_acquisition::EvidenceVaultCatalogEntry {
            evidence_id: format!("sha256-{sha256}"),
            content_path: content_addressed_path(&sha256),
            sha256,
            size_bytes,
            capture_session_id: capture_session_id.to_owned(),
            source_map_sha256: "c".repeat(64),
            source_verification_sha256: "d".repeat(64),
            capture_provenance,
        }
    }

    fn catalog_index(
        entries: Vec<crate::vault_acquisition::EvidenceVaultCatalogEntry>,
    ) -> CatalogIndex {
        let mut by_tuple = std::collections::BTreeMap::new();
        for entry in entries {
            by_tuple.insert((entry.sha256.clone(), entry.size_bytes), entry);
        }
        CatalogIndex { by_tuple }
    }

    #[test]
    fn canonical_candidate_bytes_are_stable_and_cannot_encode_approval() {
        let entry = ReviewLedgerEntry {
            form_id: "form-001".to_owned(),
            packet_id: "form-001-packet".to_owned(),
            rule_set_id: "form-001-p7.9.6.0".to_owned(),
            tracked_v1_source_set_sha256: "a".repeat(64),
            rule_set_source_state: RuleSetSourceState::Planned {
                source_set_sha256: (),
            },
            official_package_asset_id: "package-7.9.6".to_owned(),
            capture_session_id: "capture-001".to_owned(),
            source_map_sha256: "c".repeat(64),
            source_verification_sha256: "d".repeat(64),
            capture_provenance: provenance(),
            created_at_utc: "2026-07-26T01:00:00Z".to_owned(),
            review: EvidenceReview {
                status: EvidenceReviewStatus::Candidate,
                reviewed_by: None,
                reviewed_at_utc: None,
            },
            attestations: attestations(),
            derived_reviews: vec![DerivedReview {
                path: "derived/gaps.json".to_owned(),
                status: EvidenceReviewStatus::Candidate,
            }],
            source_excerpts: Vec::new(),
            capture_gaps: Vec::new(),
            expected_packet_digest_sha256: None,
        };
        let ledger = ReviewLedger {
            format: "bir-evidence-review-ledger-v1".to_owned(),
            canonicalization: "bir-json-c14n-v1".to_owned(),
            entries: vec![entry],
        };
        let first = canonical_serialize(&ledger, "test ledger").expect("serialize ledger");
        let second = canonical_serialize(&ledger, "test ledger").expect("serialize ledger again");
        assert_eq!(first, second);
        assert_eq!(sha256_hex(&first), sha256_hex(&second));
        assert!(!first.contains(&b'\r'));
        let value: Value = serde_json::from_slice(&first).expect("parse canonical ledger");
        assert_eq!(
            value.pointer("/entries/0/review/status"),
            Some(&Value::String("candidate".to_owned()))
        );
        assert_eq!(
            value.pointer("/entries/0/review/reviewed_by"),
            Some(&Value::Null)
        );
        assert_eq!(
            value.pointer("/entries/0/review/reviewed_at_utc"),
            Some(&Value::Null)
        );
        assert_eq!(
            value.pointer("/entries/0/expected_packet_digest_sha256"),
            Some(&Value::Null)
        );
    }

    #[test]
    fn catalog_identity_bijection_rejects_missing_and_extra_content() {
        let first = ("a".repeat(64), 10_u64);
        let second = ("b".repeat(64), 20_u64);
        let expected = std::collections::BTreeSet::from([first.clone()]);
        let empty = CatalogIndex {
            by_tuple: Default::default(),
        };
        let error =
            require_exact_catalog_bijection(&expected, &empty).expect_err("missing must fail");
        assert!(error.to_string().contains("missing=["));

        let mut extra = empty;
        extra.by_tuple.insert(
            second.clone(),
            crate::vault_acquisition::EvidenceVaultCatalogEntry {
                evidence_id: format!("sha256-{}", second.0),
                sha256: second.0,
                size_bytes: second.1,
                content_path: content_addressed_path(&"b".repeat(64)),
                capture_session_id: "capture".to_owned(),
                source_map_sha256: "c".repeat(64),
                source_verification_sha256: "d".repeat(64),
                capture_provenance: provenance(),
            },
        );
        let error =
            require_exact_catalog_bijection(&expected, &extra).expect_err("extra must fail");
        assert!(error.to_string().contains("extra=["));
    }

    #[test]
    fn capture_attribution_must_match_every_selected_catalog_entry() {
        let selected_entry = catalog_entry('a', 10, "selected-capture", provenance());
        let mut unrelated_provenance = provenance();
        unrelated_provenance.tool_commit = "b".repeat(40);
        let unrelated_entry =
            catalog_entry('b', 11, "unrelated-capture", unrelated_provenance.clone());
        let selected_ids = std::collections::BTreeSet::from([selected_entry.evidence_id.clone()]);
        let catalog = catalog_index(vec![selected_entry.clone(), unrelated_entry]);
        let mut candidate = input("form-001");
        candidate.capture_session_id = selected_entry.capture_session_id.clone();
        candidate.capture_provenance = selected_entry.capture_provenance.clone();
        validate_selected_capture_attribution(&candidate, &selected_ids, &catalog)
            .expect("exact selected capture attribution");

        candidate.capture_session_id = "invented-capture".to_owned();
        let error = validate_selected_capture_attribution(&candidate, &selected_ids, &catalog)
            .expect_err("invented session attribution must fail");
        assert!(
            error
                .to_string()
                .contains("must exactly match selected catalog evidence")
        );

        candidate.capture_session_id = "unrelated-capture".to_owned();
        candidate.capture_provenance = unrelated_provenance;
        let error = validate_selected_capture_attribution(&candidate, &selected_ids, &catalog)
            .expect_err("unrelated session attribution must fail");
        assert!(error.to_string().contains(&selected_entry.evidence_id));

        candidate.capture_session_id = selected_entry.capture_session_id.clone();
        candidate.capture_provenance = selected_entry.capture_provenance.clone();
        candidate.source_map_sha256 = "e".repeat(64);
        let error = validate_selected_capture_attribution(&candidate, &selected_ids, &catalog)
            .expect_err("mismatched source-map digest must fail");
        assert!(error.to_string().contains("verifier digests"));

        candidate.source_map_sha256 = selected_entry.source_map_sha256.clone();
        candidate.source_verification_sha256 = selected_entry.source_verification_sha256.clone();
        candidate.capture_provenance.finished_at_utc = "2026-07-26T00:02:00Z".to_owned();
        let error = validate_selected_capture_attribution(&candidate, &selected_ids, &catalog)
            .expect_err("mismatched selected capture provenance must fail");
        assert!(
            error
                .to_string()
                .contains("capture_provenance must exactly match")
        );
    }

    #[test]
    fn one_candidate_cannot_attribute_selected_entries_from_mixed_sessions() {
        let first = catalog_entry('a', 10, "capture-a", provenance());
        let mut second_provenance = provenance();
        second_provenance.tool_commit = "b".repeat(40);
        let second = catalog_entry('b', 11, "capture-b", second_provenance);
        let selected_ids = std::collections::BTreeSet::from([
            first.evidence_id.clone(),
            second.evidence_id.clone(),
        ]);
        let catalog = catalog_index(vec![first.clone(), second.clone()]);
        let mut candidate = input("form-001");
        candidate.capture_session_id = first.capture_session_id.clone();
        candidate.capture_provenance = first.capture_provenance.clone();
        let error = validate_selected_capture_attribution(&candidate, &selected_ids, &catalog)
            .expect_err("mixed selected capture sessions must fail");
        assert!(error.to_string().contains(&second.evidence_id));
    }

    #[test]
    fn historical_package_cannot_be_selected_for_the_current_package_identity() {
        let assets = vec![
            super::ManifestAsset {
                asset_id: "package-7.9.6".to_owned(),
                kind: "official-package-executable".to_owned(),
                sha256: "a".repeat(64),
                size_bytes: 10,
                disposition: VaultAssetDisposition::Acquirable,
            },
            super::ManifestAsset {
                asset_id: "package-7.9.5".to_owned(),
                kind: "official-package-executable".to_owned(),
                sha256: "b".repeat(64),
                size_bytes: 11,
                disposition: VaultAssetDisposition::Acquirable,
            },
        ];
        let error = select_official_package_asset(&assets, "package-7.9.5", "7.9.6.0")
            .expect_err("historical package must fail exact package binding");
        assert!(error.to_string().contains("expected `package-7.9.6`"));
        assert_eq!(
            select_official_package_asset(&assets, "package-7.9.6", "7.9.6.0")
                .expect("current package")
                .sha256,
            "a".repeat(64)
        );
    }

    #[test]
    fn capture_inputs_reject_unsorted_or_unbounded_review_locators() {
        let excerpts = vec![
            CandidateSourceExcerpt {
                excerpt_id: "z-last".to_owned(),
                upstream_evidence_id: format!("sha256-{}", "a".repeat(64)),
                excerpt_start_byte: 0,
                excerpt_end_byte: 2,
                excerpt_sha256: "b".repeat(64),
            },
            CandidateSourceExcerpt {
                excerpt_id: "a-first".to_owned(),
                upstream_evidence_id: format!("sha256-{}", "a".repeat(64)),
                excerpt_start_byte: 0,
                excerpt_end_byte: 2,
                excerpt_sha256: "c".repeat(64),
            },
        ];
        assert!(
            validate_source_excerpt_inputs(&excerpts)
                .expect_err("unordered excerpts must fail")
                .to_string()
                .contains("strictly ordered")
        );
        let gaps = vec![CandidateCaptureGap {
            gap_id: "gap".to_owned(),
            reason: "Explicit review gap.".to_owned(),
            source_evidence_ids: Vec::new(),
        }];
        assert!(
            validate_capture_gap_inputs(&gaps)
                .expect_err("source-free gap must fail")
                .to_string()
                .contains("at least one")
        );
    }

    #[test]
    fn unsafe_capture_or_reviewer_material_is_rejected() {
        for unsafe_value in [
            "C:\\Users\\person\\capture.json",
            "\\\\server\\private\\capture.json",
            "/Users/person/private/capture.json",
            "/Volumes/private/capture.json",
            "file:///Users/person/private/capture.json",
            "123-456-789",
            "TIN: 123–456–789",
            "reviewer@example.test",
            "reviewer reviewer@example.test approved",
            "sk-secret-material",
            "notes ghp_secret",
            "the password is hunter2",
            "https://example.test/submit",
            "see https://example.test/submit for captured behavior",
        ] {
            assert!(
                reject_unsafe_text(unsafe_value, "test value").is_err(),
                "{unsafe_value} must fail"
            );
        }
    }

    #[test]
    fn external_inputs_reject_closed_sensitive_roots_before_open_without_locator_echo() {
        let root = TestRoot::new("sensitive-input-policy");
        let paths = [
            root.path.join(".ssh").join("catalog.json"),
            root.path
                .join("eBIRForms")
                .join("savefile")
                .join("request.json"),
            root.path
                .join("Group Containers")
                .join("group.dev.goldcoders.bir")
                .join("bir_data.db"),
            root.path.join("taxpayer-data").join("request.json"),
        ];
        for (offset, path) in paths.iter().enumerate() {
            fs::create_dir_all(path.parent().expect("sensitive input parent"))
                .expect("create sensitive input parent");
            fs::write(path, b"private bytes that must never be parsed")
                .expect("create sensitive input");
            let callback_called = Cell::new(false);
            let label = if offset == 0 {
                "evidence vault catalog"
            } else {
                "evidence review scaffold request"
            };
            let error = open_verified_external_scaffold_input(path, label, |_| {
                callback_called.set(true);
                Ok(())
            })
            .expect_err("closed sensitive root must fail before open");
            assert!(!callback_called.get());
            let message = error.to_string();
            assert!(
                message.contains("credential/taxpayer/save/live-database"),
                "{message}"
            );
            assert!(
                !message.contains(path.to_string_lossy().as_ref()),
                "sensitive machine locator leaked in error: {message}"
            );
        }

        let hidden_by_parent = root
            .path
            .join(".ssh")
            .join("..")
            .join("apparently-safe.json");
        let error = open_verified_external_scaffold_input(
            &hidden_by_parent,
            "evidence vault catalog",
            |_| Ok(()),
        )
        .expect_err("sensitive component must be rejected before lexical normalization");
        assert!(
            error
                .to_string()
                .contains("credential/taxpayer/save/live-database")
        );
        assert!(
            !error
                .to_string()
                .contains(hidden_by_parent.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn external_input_sensitive_policy_is_rechecked_around_verified_open() {
        let root = TestRoot::new("input-policy-recheck");
        let path = root.path.join("catalog.json");
        fs::write(&path, b"{}").expect("create safe external input");
        let callback_count = Cell::new(0_usize);
        let verified =
            open_verified_external_scaffold_input(&path, "evidence vault catalog", |_| {
                callback_count.set(callback_count.get() + 1);
                Ok(())
            })
            .expect("open verified safe external input");
        assert_eq!(
            callback_count.get(),
            3,
            "resolved-path policy must run before and after handle open"
        );
        assert_eq!(
            verified.path(),
            fs::canonicalize(&path).expect("canonical safe input")
        );
    }

    #[test]
    fn external_input_capability_rejects_or_blocks_same_path_substitution() {
        let root = TestRoot::new("input-exact-path");
        let path = root.path.join("catalog.json");
        let displaced = root.path.join("catalog-displaced.json");
        fs::write(&path, b"approved").expect("create approved external input");
        let approved =
            open_verified_external_scaffold_input(&path, "evidence vault catalog", |_| Ok(()))
                .expect("approve exact external input");

        match fs::rename(&path, &displaced) {
            Ok(()) => {
                fs::write(&path, b"replacement").expect("create same-path replacement");
                read_external_bytes_bound(approved, "evidence vault catalog")
                    .expect_err("same-path replacement must not be read");
            }
            Err(_) => {
                assert_eq!(
                    read_external_bytes_bound(approved, "evidence vault catalog")
                        .expect("restrictive handle blocks substitution"),
                    b"approved"
                );
            }
        }
    }

    #[test]
    fn closed_sensitive_path_policy_covers_all_credential_and_live_data_roots() {
        for path in [
            PathBuf::from("/root/.aws/catalog.json"),
            PathBuf::from("/root/.azure/request.json"),
            PathBuf::from("/root/.gnupg/catalog.json"),
            PathBuf::from("/root/.kube/request.json"),
            PathBuf::from("/root/.docker/catalog.json"),
            PathBuf::from("/root/keychain/request.json"),
            PathBuf::from("/root/credentials/catalog.json"),
            PathBuf::from("/root/secrets/request.json"),
            PathBuf::from("/root/eBIRForms/profile/request.json"),
            PathBuf::from("/root/live-taxpayer-data/catalog.json"),
            PathBuf::from("/root/bir_data.db"),
        ] {
            let error = reject_sensitive_scaffold_input_path(&path, "external scaffold input")
                .expect_err("closed sensitive path must fail");
            assert!(
                error
                    .to_string()
                    .contains("credential/taxpayer/save/live-database")
            );
            assert!(!error.to_string().contains(path.to_string_lossy().as_ref()));
        }
    }

    #[test]
    fn line_endings_are_normalized_before_tracked_digest_binding() {
        let lf = canonical_text(b"# Evidence\n\nValue-free.\n", "evidence.md")
            .expect("canonical LF text");
        let crlf = canonical_text(b"# Evidence\r\n\r\nValue-free.\r\n", "evidence.md")
            .expect("canonical CRLF text");
        assert_eq!(lf, crlf);
        assert!(!lf.contains(&b'\r'));
    }

    #[test]
    fn output_must_be_fresh_and_outside_the_repository() {
        let root = TestRoot::new("external-output");
        let repo = root.path.join("repo");
        let external = root.path.join("external");
        fs::create_dir(&repo).expect("create repo");
        fs::create_dir(&external).expect("create external");
        let inside = repo.join("candidate.json");
        let error = validate_fresh_external_output(&repo, &inside)
            .expect_err("repo-contained output must fail");
        assert!(error.to_string().contains("outside repository"));

        let outside = external.join("candidate.json");
        let resolved =
            validate_fresh_external_output(&repo, &outside).expect("fresh external output");
        fs::write(&resolved, b"occupied").expect("occupy output");
        let error =
            validate_fresh_external_output(&repo, &outside).expect_err("overwrite must fail");
        assert!(error.to_string().contains("refusing to overwrite"));
    }

    #[test]
    fn fresh_writer_preserves_exact_bytes_and_refuses_overwrite() {
        let root = TestRoot::new("fresh-file");
        let target = root.path.join("candidate.json");
        let bytes = br#"{"candidate":true}"#;
        write_fresh_file(&target, bytes).expect("write fresh candidate ledger");
        assert_eq!(
            fs::read(&target).expect("read candidate ledger"),
            bytes.to_vec()
        );
        let error =
            write_fresh_file(&target, b"replacement").expect_err("writer must refuse overwrite");
        assert!(error.to_string().contains("refusing to overwrite"));
    }

    #[test]
    fn post_create_failure_leaves_owned_output_in_place_without_cleanup() {
        let root = TestRoot::new("post-create-failure");
        let target = root.path.join("candidate.json");
        let bytes = br#"{"candidate":true}"#;
        let error = write_fresh_file_with_hook(&target, bytes, |_| {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected post-create failure",
            ))
        })
        .expect_err("post-create failure must be reported");
        assert!(error.to_string().contains("deliberately left in place"));
        assert!(target.exists(), "owned incomplete output must remain");
    }

    #[test]
    fn racing_fresh_writers_never_overwrite_each_other() {
        let root = TestRoot::new("writer-race");
        let target = root.path.join("candidate.json");
        let left_target = target.clone();
        let left = thread::spawn(move || write_fresh_file(&left_target, br#"{"writer":"left"}"#));
        let right_target = target.clone();
        let right =
            thread::spawn(move || write_fresh_file(&right_target, br#"{"writer":"right"}"#));
        let outcomes = [
            left.join().expect("left writer did not panic"),
            right.join().expect("right writer did not panic"),
        ];
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        let installed = fs::read(&target).expect("read winning output");
        assert!(
            installed.as_slice() == br#"{"writer":"left"}"#
                || installed.as_slice() == br#"{"writer":"right"}"#
        );
    }

    #[test]
    fn hard_link_alias_after_create_is_rejected_and_neither_name_is_deleted() {
        let root = TestRoot::new("hard-link-output");
        let target = root.path.join("candidate.json");
        let alias = root.path.join("attacker-alias.json");
        let error = write_fresh_file_with_hook(&target, br#"{"candidate":true}"#, |created| {
            fs::hard_link(created, &alias)
        })
        .expect_err("hard-linked fresh output must fail closed");
        assert!(error.to_string().contains("deliberately left in place"));
        assert!(target.exists(), "owned incomplete output must remain");
        assert!(alias.exists(), "attacker alias must never be removed");
    }

    #[cfg(unix)]
    #[test]
    fn substituted_candidate_target_is_not_deleted() {
        let root = TestRoot::new("substituted-output");
        let target = root.path.join("candidate.json");
        let attacker_bytes = b"attacker-substitute";
        let error = write_fresh_file_with_hook(&target, br#"{"candidate":true}"#, |created| {
            fs::remove_file(created)?;
            fs::write(created, attacker_bytes)
        })
        .expect_err("substituted target must fail identity verification");
        assert!(error.to_string().contains("deliberately left in place"));
        assert_eq!(
            fs::read(&target).expect("substitute must remain"),
            attacker_bytes
        );
    }

    #[test]
    fn dry_run_returns_no_write_and_leaves_exact_target_absent() {
        let root = TestRoot::new("dry-run");
        let target = root.path.join("candidate.json");
        let bytes = br#"{"candidate":true}"#;
        assert!(!install_candidate_ledger(&target, bytes, true).expect("plan dry-run"));
        assert!(!target.exists());
    }

    #[test]
    fn input_identity_order_is_not_silently_sorted() {
        let forms = vec![
            super::RulesIndexEntry {
                form_id: "form-001".to_owned(),
                form_code: "F001".to_owned(),
                revision: "2018-01-01".to_owned(),
                package_version: "7.9.6.0".to_owned(),
                priority: 1,
                status: "complete".to_owned(),
                path: "forms/form-001/manifest.json".to_owned(),
            },
            super::RulesIndexEntry {
                form_id: "form-002".to_owned(),
                form_code: "F002".to_owned(),
                revision: "2018-01-01".to_owned(),
                package_version: "7.9.6.0".to_owned(),
                priority: 2,
                status: "complete".to_owned(),
                path: "forms/form-002/manifest.json".to_owned(),
            },
        ];
        let inputs = vec![input("form-002"), input("form-001")];
        let error = validate_input_bijection(&forms, &inputs)
            .expect_err("reordered identity inputs must fail");
        assert!(error.to_string().contains("exact ordered bijection"));
    }

    #[test]
    fn lexical_escape_is_rejected_before_path_resolution() {
        assert!(
            absolute_normalized(Path::new("outside/../candidate.json"))
                .expect_err("parent traversal must fail")
                .to_string()
                .contains("lexically normalized")
        );
        assert_eq!(CONTENT_ADDRESS_PREFIX, "upstream/sha256/");
        let sha256 = "ab".repeat(32);
        assert_eq!(
            content_addressed_path(&sha256),
            format!("upstream/sha256/ab/{sha256}")
        );
    }

    #[cfg(unix)]
    #[test]
    fn output_rejects_symlink_ancestor() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new("symlink-output");
        let repo = root.path.join("repo");
        let real = root.path.join("real");
        fs::create_dir(&repo).expect("create repo");
        fs::create_dir(&real).expect("create real output parent");
        let link = root.path.join("link");
        symlink(&real, &link).expect("create output symlink");
        let error = validate_fresh_external_output(&repo, &link.join("candidate.json"))
            .expect_err("symlink ancestor must fail");
        assert!(
            error.to_string().contains("symlink") || error.to_string().contains("reparse point")
        );
    }

    #[cfg(unix)]
    #[test]
    fn external_scaffold_request_rejects_symlink_input() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new("symlink-request");
        let repo = root.path.join("repo");
        fs::create_dir_all(repo.join("rules")).expect("create rules marker");
        fs::write(repo.join("rules/index.json"), b"{}").expect("create index marker");
        fs::create_dir_all(repo.join("crates/bir-rules")).expect("create crate marker");
        let real = root.path.join("request.json");
        fs::write(&real, b"{}").expect("create external request");
        let link = root.path.join("request-link.json");
        symlink(&real, &link).expect("create request symlink");
        let error = load_evidence_review_scaffold_request(&repo, &link)
            .expect_err("external request symlink must fail before read");
        assert!(
            error.to_string().contains("symlink") || error.to_string().contains("reparse point")
        );
    }

    #[cfg(windows)]
    #[test]
    fn output_rejects_reparse_or_symlink_ancestor() {
        use std::io;
        use std::os::windows::fs::symlink_dir;

        let root = TestRoot::new("reparse-output");
        let repo = root.path.join("repo");
        let real = root.path.join("real");
        fs::create_dir(&repo).expect("create repo");
        fs::create_dir(&real).expect("create real output parent");
        let link = root.path.join("link");
        match symlink_dir(&real, &link) {
            Ok(()) => {}
            Err(source)
                if source.kind() == io::ErrorKind::PermissionDenied
                    || source.kind() == io::ErrorKind::Unsupported
                    || source.raw_os_error() == Some(1314) =>
            {
                return;
            }
            Err(source) => panic!("create output symlink/reparse point: {source}"),
        }
        let error = validate_fresh_external_output(&repo, &link.join("candidate.json"))
            .expect_err("symlink/reparse ancestor must fail");
        assert!(
            error.to_string().contains("symlink") || error.to_string().contains("reparse point")
        );
    }

    #[cfg(windows)]
    #[test]
    fn external_scaffold_request_rejects_reparse_or_symlink_input() {
        use std::os::windows::fs::symlink_file;

        let root = TestRoot::new("symlink-request");
        let repo = root.path.join("repo");
        fs::create_dir_all(repo.join("rules")).expect("create rules marker");
        fs::write(repo.join("rules/index.json"), b"{}").expect("create index marker");
        fs::create_dir_all(repo.join("crates/bir-rules")).expect("create crate marker");
        let real = root.path.join("request.json");
        fs::write(&real, b"{}").expect("create external request");
        let link = root.path.join("request-link.json");
        match symlink_file(&real, &link) {
            Ok(()) => {}
            Err(source)
                if source.kind() == io::ErrorKind::PermissionDenied
                    || source.kind() == io::ErrorKind::Unsupported
                    || source.raw_os_error() == Some(1314) =>
            {
                return;
            }
            Err(source) => panic!("create request symlink/reparse point: {source}"),
        }
        let error = load_evidence_review_scaffold_request(&repo, &link)
            .expect_err("external request symlink/reparse point must fail before read");
        assert!(
            error.to_string().contains("symlink") || error.to_string().contains("reparse point")
        );
    }
}
