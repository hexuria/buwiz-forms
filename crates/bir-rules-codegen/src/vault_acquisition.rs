//! Fail-closed acquisition of the static upstream evidence vault.
//!
//! This module deliberately treats tracked v1 manifest paths as inert prose.
//! The only operative source locations come from an explicit, external,
//! canonical source map. Capture provenance is likewise supplied as canonical
//! metadata; the acquisition process never invents a command, commit, user,
//! application version, or timestamp.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
#[cfg(windows)]
use std::fs::{File, OpenOptions};
#[cfg(windows)]
use std::io::Write;
use std::path::{Component, Path, PathBuf};
// Vault write publication is supported only on Windows; the staging counter and
// the file-identity handles it needs exist only for that target.
#[cfg(windows)]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(windows)]
use same_file::Handle;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::error::{CodegenError, Result};
use crate::evidence::{EvidenceCaptureOperatingSystem, EvidenceCaptureProvenance};
use crate::files::{
    ApprovedExternalFile, ApprovedExternalRoot, ReadScope, read_external_bytes_bound,
    read_external_bytes_under, read_tracked_bytes,
};
use crate::json::{CANONICALIZATION_ID, canonical_bytes, parse_strict};
#[cfg(windows)]
use crate::path::portable_join;
use crate::path::{
    canonical_repo_root, ensure_under, is_same_or_below, is_same_path, is_symlink_or_reparse_point,
};
use crate::sensitive::reject_sensitive_text;

pub const EVIDENCE_VAULT_SOURCE_MAP_FORMAT: &str = "bir-evidence-vault-source-map-v1";
pub const EVIDENCE_VAULT_CAPTURE_METADATA_FORMAT: &str = "bir-evidence-vault-capture-metadata-v1";
pub const EVIDENCE_VAULT_CATALOG_FORMAT: &str = "bir-evidence-vault-catalog-v1";
pub const EVIDENCE_VAULT_CATALOG_FILE: &str = "vault-catalog.json";
pub const EXPECTED_V1_FORM_MANIFEST_COUNT: usize = 43;
pub const EVIDENCE_VAULT_SOURCE_VERIFICATION_DOMAIN: &str =
    "bir-evidence-vault-source-verification-v1";

const CONTENT_ADDRESS_PREFIX: &str = "upstream/sha256/";
#[cfg(any(windows, test))]
const STAGING_MARKER: &str = ".bir-vault-acquisition-staging-";
#[cfg(windows)]
static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Complete, explicit inputs for a vault acquisition or no-write plan.
#[derive(Clone, Debug)]
pub struct AcquireEvidenceVaultOptions {
    pub repo_root: PathBuf,
    pub source_map: PathBuf,
    pub capture_metadata: PathBuf,
    pub vault_root: PathBuf,
    pub dry_run: bool,
    pub(crate) read_scope: ReadScope,
}

impl AcquireEvidenceVaultOptions {
    pub fn tracked_checkout(
        repo_root: impl Into<PathBuf>,
        source_map: impl Into<PathBuf>,
        capture_metadata: impl Into<PathBuf>,
        vault_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            source_map: source_map.into(),
            capture_metadata: capture_metadata.into(),
            vault_root: vault_root.into(),
            dry_run: false,
            read_scope: ReadScope::Tracked,
        }
    }

    pub fn external_workspace(
        repo_root: impl Into<PathBuf>,
        source_map: impl Into<PathBuf>,
        capture_metadata: impl Into<PathBuf>,
        vault_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            source_map: source_map.into(),
            capture_metadata: capture_metadata.into(),
            vault_root: vault_root.into(),
            dry_run: false,
            read_scope: ReadScope::External,
        }
    }
}

/// Inputs for the no-write verification pass whose exact invocation can be
/// recorded as capture provenance without embedding a machine-local path.
#[derive(Clone, Debug)]
pub struct VerifyEvidenceVaultSourceMapOptions {
    pub repo_root: PathBuf,
    pub source_map: PathBuf,
    pub(crate) read_scope: ReadScope,
}

impl VerifyEvidenceVaultSourceMapOptions {
    pub fn tracked_checkout(repo_root: impl Into<PathBuf>, source_map: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            source_map: source_map.into(),
            read_scope: ReadScope::Tracked,
        }
    }

    pub fn external_workspace(
        repo_root: impl Into<PathBuf>,
        source_map: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            source_map: source_map.into(),
            read_scope: ReadScope::External,
        }
    }
}

/// Closed disposition shared by acquisition and packet projection.
///
/// Unknown kinds are errors rather than a fourth, permissive disposition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum VaultAssetDisposition {
    Acquirable,
    ZeroSizeProvenance,
    MetadataOnlyTaxpayerPayload,
}

/// A declared upstream identity that intentionally has no vault bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VaultAcquisitionGap {
    pub form_id: String,
    pub asset_id: String,
    pub kind: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub disposition: VaultAssetDisposition,
}

/// The existing packet factory's external catalog contract.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceVaultCatalog {
    pub format: String,
    pub canonicalization: String,
    pub entries: Vec<EvidenceVaultCatalogEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceVaultCatalogEntry {
    pub evidence_id: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub content_path: String,
    pub capture_session_id: String,
    pub source_map_sha256: String,
    pub source_verification_sha256: String,
    pub capture_provenance: EvidenceCaptureProvenance,
}

/// Canonical external map from tracked identities to explicit current-OS paths.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceVaultSourceMap {
    pub format: String,
    pub canonicalization: String,
    pub entries: Vec<EvidenceVaultSourceMapEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceVaultSourceMapEntry {
    pub form_id: String,
    pub asset_id: String,
    pub source_path: String,
}

/// Explicit capture facts copied into every unique catalog entry.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceVaultCaptureMetadata {
    pub format: String,
    pub canonicalization: String,
    pub capture_session_id: String,
    pub source_map_sha256: String,
    pub source_verification_sha256: String,
    pub capture_provenance: EvidenceCaptureProvenance,
}

/// Exact plan/result returned in both dry-run and write modes.
#[derive(Clone, Debug, Serialize)]
pub struct AcquireEvidenceVaultReport {
    pub manifest_count: usize,
    pub declared_asset_count: usize,
    pub mapped_asset_count: usize,
    pub verified_source_file_count: usize,
    pub unique_content_count: usize,
    pub deduplicated_asset_count: usize,
    pub unique_content_bytes: u64,
    pub gaps: Vec<VaultAcquisitionGap>,
    pub catalog: EvidenceVaultCatalog,
    pub catalog_sha256: String,
    pub source_map_sha256: String,
    pub source_verification_sha256: String,
    pub vault_root: PathBuf,
    pub catalog_path: PathBuf,
    pub written: bool,
}

impl AcquireEvidenceVaultReport {
    pub fn canonical_catalog_bytes(&self) -> Result<Vec<u8>> {
        canonical_serialize(&self.catalog, "evidence vault catalog")
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct VerifyEvidenceVaultSourceMapReport {
    pub manifest_count: usize,
    pub declared_asset_count: usize,
    pub mapped_asset_count: usize,
    pub verified_source_file_count: usize,
    pub unique_content_count: usize,
    pub deduplicated_asset_count: usize,
    pub unique_content_bytes: u64,
    pub gaps: Vec<VaultAcquisitionGap>,
    pub source_map_sha256: String,
    pub verification_sha256: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AssetKey {
    form_id: String,
    asset_id: String,
}

impl AssetKey {
    fn display(&self) -> String {
        format!("{}#{}", self.form_id, self.asset_id)
    }
}

#[derive(Clone, Debug)]
struct DeclaredAsset {
    key: AssetKey,
    kind: String,
    sha256: String,
    size_bytes: u64,
    disposition: VaultAssetDisposition,
}

#[derive(Clone, Debug)]
#[cfg_attr(not(windows), allow(dead_code))]
struct VerifiedContent {
    sha256: String,
    size_bytes: u64,
    source_path: PathBuf,
}

#[derive(Clone, Debug)]
struct VerifiedSourceInventory {
    manifest_count: usize,
    declared_asset_count: usize,
    mapped_asset_count: usize,
    verified_source_file_count: usize,
    unique_content_bytes: u64,
    source_map_sha256: String,
    verification_sha256: String,
    gaps: Vec<VaultAcquisitionGap>,
    content: BTreeMap<(String, u64), VerifiedContent>,
}

#[derive(Clone, Debug)]
#[cfg_attr(not(windows), allow(dead_code))]
struct AcquisitionPlan {
    repo_root: PathBuf,
    manifest_count: usize,
    declared_asset_count: usize,
    mapped_asset_count: usize,
    verified_source_file_count: usize,
    unique_content_bytes: u64,
    source_map_sha256: String,
    source_verification_sha256: String,
    gaps: Vec<VaultAcquisitionGap>,
    catalog: EvidenceVaultCatalog,
    catalog_bytes: Vec<u8>,
    content: BTreeMap<(String, u64), VerifiedContent>,
}

/// Verify all explicit inputs and either install a fresh vault atomically or
/// return the exact no-write plan.
pub fn acquire_evidence_vault(
    options: &AcquireEvidenceVaultOptions,
) -> Result<AcquireEvidenceVaultReport> {
    #[cfg(not(windows))]
    if !options.dry_run {
        return Err(CodegenError::new(
            "evidence vault write publication is supported only on Windows; use --dry-run or verify-evidence-vault-source-map on this platform",
        ));
    }

    let repo_root = canonical_repo_root(&options.repo_root)?;
    let vault_root = validate_fresh_external_vault_root(&repo_root, &options.vault_root)?;

    let (source_map_path, source_map): (_, EvidenceVaultSourceMap) =
        load_external_canonical_json(&repo_root, &options.source_map, "vault source map")?;
    validate_source_map_header(&source_map)?;
    let (_capture_metadata_path, capture_metadata): (_, EvidenceVaultCaptureMetadata) =
        load_external_canonical_json(
            &repo_root,
            &options.capture_metadata,
            "vault capture metadata",
        )?;
    validate_capture_metadata(&capture_metadata)?;

    let (manifest_count, declared_assets) =
        load_declared_manifest_assets(&repo_root, options.read_scope)?;
    let plan = build_acquisition_plan(
        &repo_root,
        &source_map_path,
        manifest_count,
        declared_assets,
        source_map,
        capture_metadata,
    )?;

    if !options.dry_run {
        install_plan_atomically(&vault_root, &plan)?;
    }

    let unique_content_count = plan.catalog.entries.len();
    Ok(AcquireEvidenceVaultReport {
        manifest_count: plan.manifest_count,
        declared_asset_count: plan.declared_asset_count,
        mapped_asset_count: plan.mapped_asset_count,
        verified_source_file_count: plan.verified_source_file_count,
        unique_content_count,
        deduplicated_asset_count: plan.mapped_asset_count.saturating_sub(unique_content_count),
        unique_content_bytes: plan.unique_content_bytes,
        gaps: plan.gaps,
        catalog: plan.catalog,
        catalog_sha256: sha256_hex(&plan.catalog_bytes),
        source_map_sha256: plan.source_map_sha256,
        source_verification_sha256: plan.source_verification_sha256,
        catalog_path: vault_root.join(EVIDENCE_VAULT_CATALOG_FILE),
        vault_root,
        written: !options.dry_run,
    })
}

/// Re-hash every explicit source-map asset against all 43 tracked manifests.
///
/// This pass performs no writes and requires no capture metadata. Its exact
/// portable invocation can therefore be recorded after it completes, avoiding
/// the provenance cycle where metadata would otherwise need its own finish
/// timestamp before the verified capture command had run.
pub fn verify_evidence_vault_source_map(
    options: &VerifyEvidenceVaultSourceMapOptions,
) -> Result<VerifyEvidenceVaultSourceMapReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let (_source_map_path, source_map): (_, EvidenceVaultSourceMap) =
        load_external_canonical_json(&repo_root, &options.source_map, "vault source map")?;
    validate_source_map_header(&source_map)?;
    let (manifest_count, declared_assets) =
        load_declared_manifest_assets(&repo_root, options.read_scope)?;
    let inventory =
        build_verified_source_inventory(&repo_root, manifest_count, declared_assets, source_map)?;
    let unique_content_count = inventory.content.len();
    Ok(VerifyEvidenceVaultSourceMapReport {
        manifest_count: inventory.manifest_count,
        declared_asset_count: inventory.declared_asset_count,
        mapped_asset_count: inventory.mapped_asset_count,
        verified_source_file_count: inventory.verified_source_file_count,
        unique_content_count,
        deduplicated_asset_count: inventory
            .mapped_asset_count
            .saturating_sub(unique_content_count),
        unique_content_bytes: inventory.unique_content_bytes,
        gaps: inventory.gaps,
        source_map_sha256: inventory.source_map_sha256,
        verification_sha256: inventory.verification_sha256,
    })
}

/// Apply the single closed kind/size policy used by vault acquisition.
pub fn vault_asset_disposition(kind: &str, size_bytes: u64) -> Result<VaultAssetDisposition> {
    validate_portable_identifier(kind, "official asset kind")?;
    if is_taxpayer_payload_kind(kind) {
        return Ok(VaultAssetDisposition::MetadataOnlyTaxpayerPayload);
    }
    if kind == "prior-reviewed-repository-provenance" {
        if size_bytes == 0 {
            return Ok(VaultAssetDisposition::ZeroSizeProvenance);
        }
        return Err(CodegenError::new(
            "prior-reviewed-repository-provenance must remain a zero-size metadata pin",
        ));
    }
    if !ACQUIRABLE_OFFICIAL_ASSET_KINDS.contains(&kind) {
        return Err(CodegenError::new(format!(
            "official asset kind `{kind}` has no reviewed vault disposition"
        )));
    }
    if size_bytes == 0 {
        Ok(VaultAssetDisposition::ZeroSizeProvenance)
    } else {
        Ok(VaultAssetDisposition::Acquirable)
    }
}

const ACQUIRABLE_OFFICIAL_ASSET_KINDS: &[&str] = &[
    "official-annex-pdf",
    "official-bir-issuance",
    "official-circular-pdf",
    "official-form-pdf",
    "official-form-pdf-comparator",
    "official-form-pdf-earlier-revision",
    "official-form-pdf-later-revision",
    "official-guide-pdf",
    "official-guide-pdf-comparator",
    "official-guidelines-pdf",
    "official-mandatory-attachment-pdf",
    "official-package-executable",
    "official-package-helper-executable",
    "official-package-release-notes",
    "official-package-xml",
    "official-runtime-data",
    "official-runtime-help",
    "official-runtime-help-legacy",
    "official-runtime-help-predecessor",
    "runtime-extracted-help-hta",
    "runtime-extracted-hta",
    "runtime-extracted-hta-legacy",
    "runtime-extracted-hta-predecessor",
    "runtime-extracted-javascript",
    "runtime-extracted-vbscript",
    "runtime-help",
    "runtime-help-mismatch",
    "runtime-javascript",
    "runtime-lookup-catalog",
    "runtime-vbscript",
    "shared-javascript",
    "shared-vbscript",
    "virtualized-helper-executable",
];

fn is_taxpayer_payload_kind(kind: &str) -> bool {
    kind.contains("dummy-profile")
        || kind.contains("final-copy")
        || kind.contains("taxpayer")
        || kind.contains("savefile")
        || kind.contains("return-payload")
        || kind.contains("submission-payload")
        || kind == "saved-return"
        || kind.starts_with("saved-return-")
        || kind.ends_with("-save")
        || kind.contains("-saved-")
}

fn build_acquisition_plan(
    repo_root: &Path,
    source_map_path: &Path,
    manifest_count: usize,
    declared_assets: Vec<DeclaredAsset>,
    source_map: EvidenceVaultSourceMap,
    capture_metadata: EvidenceVaultCaptureMetadata,
) -> Result<AcquisitionPlan> {
    validate_source_verifier_binding(
        &capture_metadata.capture_provenance,
        repo_root,
        source_map_path,
    )?;
    let inventory =
        build_verified_source_inventory(repo_root, manifest_count, declared_assets, source_map)?;
    if capture_metadata.source_map_sha256 != inventory.source_map_sha256
        || capture_metadata.source_verification_sha256 != inventory.verification_sha256
    {
        return Err(CodegenError::new(format!(
            "capture metadata does not bind this verified source map (metadata map {}/verification {}, observed map {}/verification {})",
            capture_metadata.source_map_sha256,
            capture_metadata.source_verification_sha256,
            inventory.source_map_sha256,
            inventory.verification_sha256,
        )));
    }
    let VerifiedSourceInventory {
        manifest_count,
        declared_asset_count,
        mapped_asset_count,
        verified_source_file_count,
        unique_content_bytes,
        source_map_sha256,
        verification_sha256: source_verification_sha256,
        gaps,
        content,
    } = inventory;

    let mut catalog_entries = Vec::with_capacity(content.len());
    for ((sha256, size_bytes), verified) in &content {
        debug_assert_eq!(sha256, &verified.sha256);
        debug_assert_eq!(size_bytes, &verified.size_bytes);
        catalog_entries.push(EvidenceVaultCatalogEntry {
            evidence_id: format!("sha256-{sha256}"),
            sha256: sha256.clone(),
            size_bytes: *size_bytes,
            content_path: content_addressed_path(sha256),
            capture_session_id: capture_metadata.capture_session_id.clone(),
            source_map_sha256: capture_metadata.source_map_sha256.clone(),
            source_verification_sha256: capture_metadata.source_verification_sha256.clone(),
            capture_provenance: capture_metadata.capture_provenance.clone(),
        });
    }
    catalog_entries.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    let catalog = EvidenceVaultCatalog {
        format: EVIDENCE_VAULT_CATALOG_FORMAT.to_owned(),
        canonicalization: CANONICALIZATION_ID.to_owned(),
        entries: catalog_entries,
    };
    validate_emitted_catalog(&catalog)?;
    let catalog_bytes = canonical_serialize(&catalog, "evidence vault catalog")?;

    Ok(AcquisitionPlan {
        repo_root: repo_root.to_path_buf(),
        manifest_count,
        declared_asset_count,
        mapped_asset_count,
        verified_source_file_count,
        unique_content_bytes,
        source_map_sha256,
        source_verification_sha256,
        gaps,
        catalog,
        catalog_bytes,
        content,
    })
}

fn build_verified_source_inventory(
    repo_root: &Path,
    manifest_count: usize,
    declared_assets: Vec<DeclaredAsset>,
    source_map: EvidenceVaultSourceMap,
) -> Result<VerifiedSourceInventory> {
    let source_map_bytes = canonical_serialize(&source_map, "evidence vault source map")?;
    let source_map_sha256 = sha256_hex(&source_map_bytes);
    let declared_asset_count = declared_assets.len();
    let declared_by_key: BTreeMap<AssetKey, &DeclaredAsset> = declared_assets
        .iter()
        .map(|asset| (asset.key.clone(), asset))
        .collect();
    if declared_by_key.len() != declared_asset_count {
        return Err(CodegenError::new(
            "tracked manifests contain duplicate form/asset identities",
        ));
    }

    let mut expected_map_keys = BTreeSet::new();
    let mut gaps = Vec::new();
    for asset in &declared_assets {
        match asset.disposition {
            VaultAssetDisposition::Acquirable => {
                expected_map_keys.insert(asset.key.clone());
            }
            VaultAssetDisposition::ZeroSizeProvenance
            | VaultAssetDisposition::MetadataOnlyTaxpayerPayload => {
                gaps.push(VaultAcquisitionGap {
                    form_id: asset.key.form_id.clone(),
                    asset_id: asset.key.asset_id.clone(),
                    kind: asset.kind.clone(),
                    sha256: asset.sha256.clone(),
                    size_bytes: asset.size_bytes,
                    disposition: asset.disposition,
                });
            }
        }
    }
    gaps.sort_by(|left, right| {
        (&left.form_id, &left.asset_id).cmp(&(&right.form_id, &right.asset_id))
    });

    let mut source_by_key = BTreeMap::<AssetKey, EvidenceVaultSourceMapEntry>::new();
    for entry in source_map.entries {
        validate_portable_identifier(&entry.form_id, "source-map form_id")?;
        validate_portable_identifier(&entry.asset_id, "source-map asset_id")?;
        let key = AssetKey {
            form_id: entry.form_id.clone(),
            asset_id: entry.asset_id.clone(),
        };
        if let Some(asset) = declared_by_key.get(&key)
            && asset.disposition != VaultAssetDisposition::Acquirable
        {
            return Err(CodegenError::new(format!(
                "source-map entry `{}` targets a metadata-only `{}` asset and must not supply bytes",
                key.display(),
                asset.kind
            )));
        }
        if source_by_key.insert(key.clone(), entry).is_some() {
            return Err(CodegenError::new(format!(
                "source map contains duplicate asset identity `{}`",
                key.display()
            )));
        }
    }

    let actual_map_keys: BTreeSet<AssetKey> = source_by_key.keys().cloned().collect();
    let missing: Vec<String> = expected_map_keys
        .difference(&actual_map_keys)
        .map(AssetKey::display)
        .collect();
    let extra: Vec<String> = actual_map_keys
        .difference(&expected_map_keys)
        .map(AssetKey::display)
        .collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(CodegenError::new(format!(
            "source map must exactly cover acquirable nonzero manifest assets; missing=[{}] extra=[{}]",
            missing.join(", "),
            extra.join(", ")
        )));
    }

    let mut verified_paths = BTreeMap::<PathBuf, (String, u64)>::new();
    let mut content = BTreeMap::<(String, u64), VerifiedContent>::new();
    let mut size_by_hash = BTreeMap::<String, u64>::new();
    for key in &expected_map_keys {
        let asset = declared_by_key
            .get(key)
            .expect("expected source-map key came from declared asset");
        let map_entry = source_by_key
            .get(key)
            .expect("source-map set equality proved the entry exists");
        let source_path = canonical_source_asset_file(
            repo_root,
            Path::new(&map_entry.source_path),
            &key.display(),
        )?;
        let observed = if let Some(observed) = verified_paths.get(&source_path) {
            observed.clone()
        } else {
            let observed = hash_regular_file(repo_root, &source_path, "vault source asset", None)?;
            verified_paths.insert(source_path.clone(), observed.clone());
            observed
        };
        if observed.1 != asset.size_bytes || observed.0 != asset.sha256 {
            return Err(CodegenError::new(format!(
                "source bytes for `{}` do not match manifest identity: expected {} bytes/{} observed {} bytes/{}",
                key.display(),
                asset.size_bytes,
                asset.sha256,
                observed.1,
                observed.0
            )));
        }
        if let Some(previous_size) = size_by_hash.insert(asset.sha256.clone(), asset.size_bytes)
            && previous_size != asset.size_bytes
        {
            return Err(CodegenError::new(format!(
                "manifest sha256 `{}` is paired with conflicting sizes {previous_size} and {}",
                asset.sha256, asset.size_bytes
            )));
        }
        content
            .entry((asset.sha256.clone(), asset.size_bytes))
            .or_insert_with(|| VerifiedContent {
                sha256: asset.sha256.clone(),
                size_bytes: asset.size_bytes,
                source_path,
            });
    }
    if content.is_empty() {
        return Err(CodegenError::new(
            "vault acquisition has no acquirable nonzero official package assets",
        ));
    }

    let mut unique_content_bytes = 0_u64;
    for (_sha256, size_bytes) in content.keys() {
        unique_content_bytes = unique_content_bytes
            .checked_add(*size_bytes)
            .ok_or_else(|| CodegenError::new("unique vault content byte total exceeds u64"))?;
    }

    let mut verification_entries = Vec::<(String, Vec<u8>)>::new();
    verification_entries.push(("source-map.json".to_owned(), source_map_bytes));
    for key in &expected_map_keys {
        let asset = declared_by_key
            .get(key)
            .expect("verified source key came from a declared asset");
        verification_entries.push((
            format!("assets/{}#{}", key.form_id, key.asset_id),
            format!("{}:{}", asset.sha256, asset.size_bytes).into_bytes(),
        ));
    }
    let verification_sha256 = crate::hash::digest_entries(
        EVIDENCE_VAULT_SOURCE_VERIFICATION_DOMAIN,
        verification_entries
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    );

    Ok(VerifiedSourceInventory {
        manifest_count,
        declared_asset_count,
        mapped_asset_count: expected_map_keys.len(),
        verified_source_file_count: verified_paths.len(),
        unique_content_bytes,
        source_map_sha256,
        verification_sha256,
        gaps,
        content,
    })
}

fn load_declared_manifest_assets(
    repo_root: &Path,
    read_scope: ReadScope,
) -> Result<(usize, Vec<DeclaredAsset>)> {
    let approved_workspace = match read_scope {
        ReadScope::Tracked => None,
        ReadScope::External => {
            let expected = repo_root.to_path_buf();
            Some(ApprovedExternalRoot::capture(
                repo_root,
                "external rules workspace root",
                |resolved| {
                    if !is_same_path(resolved, &expected) {
                        return Err(CodegenError::new(format!(
                            "external rules workspace root `{}` resolved to a different canonical directory `{}`",
                            expected.display(),
                            resolved.display()
                        )));
                    }
                    Ok(())
                },
            )?)
        }
    };
    let forms_root_path = repo_root.join("rules/forms");
    let forms_root = canonical_real_directory(&forms_root_path, "tracked v1 forms root")?;
    ensure_under(repo_root, &forms_root, "tracked v1 forms root")?;
    let mut entries = fs::read_dir(&forms_root)
        .map_err(|source| CodegenError::io("read tracked v1 forms root", &forms_root, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CodegenError::io("read tracked v1 form entry", &forms_root, source))?;
    entries.sort_by_key(|entry| entry.file_name());
    if entries.len() != EXPECTED_V1_FORM_MANIFEST_COUNT {
        return Err(CodegenError::new(format!(
            "vault acquisition requires exactly {EXPECTED_V1_FORM_MANIFEST_COUNT} tracked v1 form directories; found {}",
            entries.len()
        )));
    }

    let mut assets = Vec::new();
    let mut form_ids = BTreeSet::new();
    for entry in entries {
        let form_root = entry.path();
        let metadata = fs::symlink_metadata(&form_root).map_err(|source| {
            CodegenError::io("inspect tracked v1 form root", &form_root, source)
        })?;
        if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
            return Err(CodegenError::new(format!(
                "tracked v1 form entry `{}` must be a real directory",
                form_root.display()
            )));
        }
        let form_id = entry.file_name().into_string().map_err(|_| {
            CodegenError::new(format!(
                "tracked v1 form directory `{}` is not valid UTF-8",
                form_root.display()
            ))
        })?;
        validate_portable_identifier(&form_id, "tracked v1 form_id")?;
        if !form_ids.insert(form_id.clone()) {
            return Err(CodegenError::new(format!(
                "duplicate tracked v1 form_id `{form_id}`"
            )));
        }
        let manifest_path = form_root.join("manifest.json");
        let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|source| {
            CodegenError::io("inspect tracked v1 manifest", &manifest_path, source)
        })?;
        if is_symlink_or_reparse_point(&manifest_metadata) || !manifest_metadata.is_file() {
            return Err(CodegenError::new(format!(
                "tracked v1 manifest `{}` must be a real regular file",
                manifest_path.display()
            )));
        }
        let manifest_bytes = match read_scope {
            ReadScope::Tracked => read_tracked_bytes(&manifest_path)?,
            ReadScope::External => read_external_bytes_under(
                approved_workspace
                    .as_ref()
                    .expect("external scope captured the exact workspace root"),
                &manifest_path,
                "external tracked v1 manifest",
            )?,
        };
        let manifest = parse_strict(&manifest_bytes, &manifest_path)?.into_serde();
        let manifest_object = manifest.as_object().ok_or_else(|| {
            CodegenError::new(format!(
                "tracked v1 manifest `{}` must be a JSON object",
                manifest_path.display()
            ))
        })?;
        let declared_form_id = required_string(manifest_object, "form_id", "tracked v1 manifest")?;
        if declared_form_id != form_id {
            return Err(CodegenError::new(format!(
                "tracked v1 manifest form_id `{declared_form_id}` does not match directory `{form_id}`"
            )));
        }
        let official_assets =
            required_array(manifest_object, "official_assets", "tracked v1 manifest")?;
        let mut asset_ids = BTreeSet::new();
        for (index, value) in official_assets.iter().enumerate() {
            let object = value.as_object().ok_or_else(|| {
                CodegenError::new(format!(
                    "{form_id} official_assets[{index}] must be an object"
                ))
            })?;
            // `path` is intentionally never accessed. It is machine-local prose.
            let asset_id = required_string(object, "asset_id", "official asset")?.to_owned();
            validate_portable_identifier(&asset_id, "official asset_id")?;
            if !asset_ids.insert(asset_id.clone()) {
                return Err(CodegenError::new(format!(
                    "{form_id} contains duplicate official asset_id `{asset_id}`"
                )));
            }
            let kind = required_string(object, "kind", "official asset")?.to_owned();
            let sha256 = required_string(object, "sha256", "official asset")?.to_owned();
            validate_sha256(
                &sha256,
                &format!("{form_id} official asset `{asset_id}` sha256"),
            )?;
            let size_bytes = object.get("size").and_then(Value::as_u64).ok_or_else(|| {
                CodegenError::new(format!(
                    "{form_id} official asset `{asset_id}` size must be a non-negative integer"
                ))
            })?;
            let disposition = vault_asset_disposition(&kind, size_bytes)?;
            assets.push(DeclaredAsset {
                key: AssetKey {
                    form_id: form_id.clone(),
                    asset_id,
                },
                kind,
                sha256,
                size_bytes,
                disposition,
            });
        }
    }
    assets.sort_by(|left, right| left.key.cmp(&right.key));
    Ok((form_ids.len(), assets))
}

fn validate_source_map_header(source_map: &EvidenceVaultSourceMap) -> Result<()> {
    if source_map.format != EVIDENCE_VAULT_SOURCE_MAP_FORMAT
        || source_map.canonicalization != CANONICALIZATION_ID
    {
        return Err(CodegenError::new(format!(
            "vault source map must use `{EVIDENCE_VAULT_SOURCE_MAP_FORMAT}` and `{CANONICALIZATION_ID}`"
        )));
    }
    if source_map.entries.is_empty() {
        return Err(CodegenError::new(
            "vault source map entries must not be empty",
        ));
    }
    Ok(())
}

pub(crate) fn validate_capture_metadata(metadata: &EvidenceVaultCaptureMetadata) -> Result<()> {
    if metadata.format != EVIDENCE_VAULT_CAPTURE_METADATA_FORMAT
        || metadata.canonicalization != CANONICALIZATION_ID
    {
        return Err(CodegenError::new(format!(
            "vault capture metadata must use `{EVIDENCE_VAULT_CAPTURE_METADATA_FORMAT}` and `{CANONICALIZATION_ID}`"
        )));
    }
    validate_portable_identifier(
        &metadata.capture_session_id,
        "capture metadata capture_session_id",
    )?;
    validate_sha256(
        &metadata.source_map_sha256,
        "capture metadata source_map_sha256",
    )?;
    validate_sha256(
        &metadata.source_verification_sha256,
        "capture metadata source_verification_sha256",
    )?;
    validate_source_verifier_provenance(&metadata.capture_provenance)
}

pub(crate) fn validate_capture_provenance(provenance: &EvidenceCaptureProvenance) -> Result<()> {
    if provenance.tool_commit.len() != 40
        || !provenance
            .tool_commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CodegenError::new(
            "capture provenance tool_commit must be a supplied full 40-character lowercase hexadecimal commit",
        ));
    }
    if provenance.command_argv.is_empty() || provenance.command_argv.len() > 256 {
        return Err(CodegenError::new(
            "capture provenance command_argv must explicitly contain 1..=256 arguments",
        ));
    }
    for argument in &provenance.command_argv {
        if argument.len() > 4_096 || argument.is_empty() || argument.chars().any(char::is_control) {
            return Err(CodegenError::new(
                "capture provenance arguments must be non-empty, bounded, and control-free",
            ));
        }
        reject_machine_locator(argument, "capture provenance argument")?;
        reject_sensitive_text(argument, "capture provenance argument")?;
    }
    for (label, value) in [
        (
            "capture_provenance.capture_tool_version",
            provenance.capture_tool_version.as_str(),
        ),
        (
            "capture_provenance.windows_version",
            provenance.windows_version.as_str(),
        ),
        (
            "capture_provenance.official_app_version",
            provenance.official_app_version.as_str(),
        ),
    ] {
        validate_safe_human_text(value, label)?;
    }
    if provenance.operating_system != EvidenceCaptureOperatingSystem::Windows {
        return Err(CodegenError::new(
            "vault capture provenance operating_system must be explicitly `windows`",
        ));
    }
    validate_utc_timestamp(
        &provenance.started_at_utc,
        "capture_provenance.started_at_utc",
    )?;
    validate_utc_timestamp(
        &provenance.finished_at_utc,
        "capture_provenance.finished_at_utc",
    )?;
    if provenance.finished_at_utc < provenance.started_at_utc {
        return Err(CodegenError::new(
            "capture provenance finish must not precede start",
        ));
    }
    Ok(())
}

pub(crate) fn validate_source_verifier_provenance(
    provenance: &EvidenceCaptureProvenance,
) -> Result<()> {
    validate_capture_provenance(provenance)?;
    let [program, command, flag, source_map] = provenance.command_argv.as_slice() else {
        return Err(CodegenError::new(
            "vault capture provenance must record exactly `bir-rules-codegen verify-evidence-vault-source-map --source-map <portable-relative-external-json>`",
        ));
    };
    if program != "bir-rules-codegen"
        || command != "verify-evidence-vault-source-map"
        || flag != "--source-map"
    {
        return Err(CodegenError::new(
            "vault capture provenance must record the exact source-map verifier command",
        ));
    }
    validate_portable_external_source_map_argument(source_map)
}

fn validate_portable_external_source_map_argument(argument: &str) -> Result<()> {
    if argument.contains('\\')
        || argument.starts_with('/')
        || argument.contains(':')
        || !argument.ends_with(".json")
    {
        return Err(CodegenError::new(
            "source-map verifier argument must be a portable `/`-separated relative external JSON path",
        ));
    }
    let components = argument.split('/').collect::<Vec<_>>();
    let parent_count = components
        .iter()
        .take_while(|component| **component == "..")
        .count();
    if parent_count == 0
        || parent_count == components.len()
        || components[parent_count..].iter().any(|component| {
            component.is_empty()
                || *component == "."
                || *component == ".."
                || !component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
    {
        return Err(CodegenError::new(
            "source-map verifier argument must leave the repository through leading `..` components and then use portable path components",
        ));
    }
    Ok(())
}

fn validate_source_verifier_binding(
    provenance: &EvidenceCaptureProvenance,
    repo_root: &Path,
    source_map_path: &Path,
) -> Result<()> {
    validate_source_verifier_provenance(provenance)?;
    let source_map_argument = provenance
        .command_argv
        .get(3)
        .expect("exact verifier provenance has four arguments");
    let mut recorded_path = repo_root.to_path_buf();
    for component in source_map_argument.split('/') {
        if component == ".." {
            if !recorded_path.pop() {
                return Err(CodegenError::new(
                    "recorded source-map verifier path escapes its filesystem root",
                ));
            }
        } else {
            recorded_path.push(component);
        }
    }
    require_absolute_normalized(&recorded_path, "recorded source-map verifier input")?;
    let expected_source_map = source_map_path.to_path_buf();
    let recorded = ApprovedExternalFile::capture(
        &recorded_path,
        "recorded source-map verifier input",
        |resolved| {
            if !is_same_path(resolved, &expected_source_map) {
                return Err(CodegenError::new(
                    "capture provenance source-map argument does not resolve to the source map being acquired",
                ));
            }
            if is_same_or_below(repo_root, resolved) {
                return Err(CodegenError::new(
                    "recorded source-map verifier input must remain outside the repository",
                ));
            }
            reject_sensitive_path(resolved, "recorded source-map verifier input")?;
            Ok(())
        },
    )?;
    if !is_same_path(recorded.path(), source_map_path) {
        return Err(CodegenError::new(
            "capture provenance source-map argument does not resolve to the source map being acquired",
        ));
    }
    Ok(())
}

fn validate_emitted_catalog(catalog: &EvidenceVaultCatalog) -> Result<()> {
    if catalog.format != EVIDENCE_VAULT_CATALOG_FORMAT
        || catalog.canonicalization != CANONICALIZATION_ID
        || catalog.entries.is_empty()
    {
        return Err(CodegenError::new(
            "generated evidence vault catalog has an invalid header or no entries",
        ));
    }
    let mut previous: Option<&str> = None;
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut capture_binding: Option<(&str, &str, &str)> = None;
    for entry in &catalog.entries {
        validate_portable_identifier(&entry.evidence_id, "catalog evidence_id")?;
        validate_sha256(&entry.sha256, "catalog sha256")?;
        validate_sha256(&entry.source_map_sha256, "catalog source_map_sha256")?;
        validate_sha256(
            &entry.source_verification_sha256,
            "catalog source_verification_sha256",
        )?;
        if entry.size_bytes == 0 {
            return Err(CodegenError::new(
                "catalog must not turn zero-size provenance into a vault file",
            ));
        }
        if previous.is_some_and(|value| value >= entry.evidence_id.as_str()) {
            return Err(CodegenError::new(
                "catalog entries must be strictly ordered by evidence_id",
            ));
        }
        previous = Some(&entry.evidence_id);
        if !ids.insert(entry.evidence_id.as_str()) {
            return Err(CodegenError::new(format!(
                "duplicate catalog evidence_id `{}`",
                entry.evidence_id
            )));
        }
        let expected_path = content_addressed_path(&entry.sha256);
        if entry.content_path != expected_path || !paths.insert(entry.content_path.as_str()) {
            return Err(CodegenError::new(format!(
                "catalog entry `{}` has a non-content-addressed or duplicate path",
                entry.evidence_id
            )));
        }
        validate_portable_identifier(&entry.capture_session_id, "catalog capture_session_id")?;
        validate_source_verifier_provenance(&entry.capture_provenance)?;
        let observed_binding = (
            entry.capture_session_id.as_str(),
            entry.source_map_sha256.as_str(),
            entry.source_verification_sha256.as_str(),
        );
        if capture_binding.is_some_and(|expected| expected != observed_binding) {
            return Err(CodegenError::new(
                "catalog entries must share one capture session and exact source-map verification binding",
            ));
        }
        capture_binding = Some(observed_binding);
    }
    let serialized = serde_json::to_value(catalog).map_err(|source| {
        CodegenError::with_source("serialize generated vault catalog for path audit", source)
    })?;
    reject_machine_strings(&serialized, "generated vault catalog")
}

#[cfg(not(windows))]
fn install_plan_atomically(_target: &Path, _plan: &AcquisitionPlan) -> Result<()> {
    Err(CodegenError::new(
        "evidence vault write publication is supported only on Windows; use --dry-run or verify-evidence-vault-source-map on this platform",
    ))
}

#[cfg(windows)]
fn install_plan_atomically(target: &Path, plan: &AcquisitionPlan) -> Result<()> {
    if target.exists() {
        return Err(CodegenError::new(format!(
            "vault root `{}` already exists; refusing to overwrite",
            target.display()
        )));
    }
    reject_symlink_ancestors(target, "vault root")?;
    let parent = target.parent().ok_or_else(|| {
        CodegenError::new(format!("vault root `{}` has no parent", target.display()))
    })?;
    let parent = canonical_real_directory(parent, "vault parent")?;
    let parent_identity = Handle::from_path(&parent)
        .map_err(|source| CodegenError::io("identify vault parent", &parent, source))?;
    let target_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CodegenError::new("vault root final component must be valid UTF-8"))?;
    let staging = parent.join(format!(
        ".{target_name}{STAGING_MARKER}{}-{}",
        std::process::id(),
        STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    if staging.exists() {
        return Err(CodegenError::new(format!(
            "owned vault staging path unexpectedly exists: `{}`",
            staging.display()
        )));
    }
    fs::create_dir(&staging)
        .map_err(|source| CodegenError::io("create vault staging root", &staging, source))?;
    let staging_identity = Handle::from_path(&staging)
        .map_err(|source| CodegenError::io("identify vault staging root", &staging, source))?;

    let operation = (|| {
        for entry in &plan.catalog.entries {
            let tuple = (entry.sha256.clone(), entry.size_bytes);
            let verified = plan.content.get(&tuple).ok_or_else(|| {
                CodegenError::new(format!(
                    "internal acquisition plan is missing catalog content `{}`",
                    entry.evidence_id
                ))
            })?;
            let destination = portable_join(&staging, &entry.content_path, "vault content path")?;
            ensure_under(&staging, &destination, "vault content file")?;
            let destination_parent = destination
                .parent()
                .expect("portable content address has a parent");
            fs::create_dir_all(destination_parent).map_err(|source| {
                CodegenError::io(
                    "create content-addressed vault directory",
                    destination_parent,
                    source,
                )
            })?;
            copy_verified_regular_file(&plan.repo_root, verified, &destination)?;
        }

        let catalog_path = staging.join(EVIDENCE_VAULT_CATALOG_FILE);
        let mut catalog_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&catalog_path)
            .map_err(|source| {
                CodegenError::io("create evidence vault catalog", &catalog_path, source)
            })?;
        catalog_file
            .write_all(&plan.catalog_bytes)
            .map_err(|source| {
                CodegenError::io("write evidence vault catalog", &catalog_path, source)
            })?;
        catalog_file.sync_all().map_err(|source| {
            CodegenError::io("sync evidence vault catalog", &catalog_path, source)
        })?;
        drop(catalog_file);

        sync_directory(&staging)?;
        verify_staged_plan(&staging, plan)?;
        let current_parent = Handle::from_path(&parent).map_err(|source| {
            CodegenError::io(
                "reidentify vault parent before publication",
                &parent,
                source,
            )
        })?;
        if current_parent != parent_identity {
            return Err(CodegenError::new(
                "vault parent changed during construction; the incomplete staging tree was left in place for manual inspection",
            ));
        }
        if target.exists() {
            return Err(CodegenError::new(format!(
                "vault root `{}` appeared during construction; refusing to overwrite",
                target.display()
            )));
        }
        publish_directory_no_replace(&staging, target)?;
        let published_parent = Handle::from_path(&parent).map_err(|source| {
            CodegenError::io("reidentify vault parent after publication", &parent, source)
        })?;
        let published_target = Handle::from_path(target).map_err(|source| {
            CodegenError::io("identify published evidence vault", target, source)
        })?;
        if published_parent != parent_identity || published_target != staging_identity {
            return Err(CodegenError::new(
                "evidence vault publication completed but path identity could not be proven; treat the target as untrusted and do not delete it automatically",
            ));
        }
        if let Err(error) = verify_staged_plan(target, plan) {
            return Err(CodegenError::new(format!(
                "evidence vault publication completed but post-publication verification failed; treat the target as untrusted and do not delete it automatically: {error}"
            )));
        }
        let _ = sync_directory(&parent);
        Ok(())
    })();

    operation.map_err(|error| {
        if staging.exists() {
            CodegenError::new(format!(
                "{error}; incomplete vault staging was left at `{}` to avoid deleting a concurrently substituted path",
                staging.display()
            ))
        } else {
            error
        }
    })
}

/// Publish a same-parent staging directory atomically without replacing any
/// target that appeared after validation.
///
/// `std::fs::rename` has replace-existing semantics on Unix and therefore
/// cannot implement this boundary. Vault writes are intentionally Windows-only
/// because Windows directory rename fails when `target` already exists.
#[cfg(windows)]
fn publish_directory_no_replace(staging: &Path, target: &Path) -> Result<()> {
    fs::rename(staging, target).map_err(|source| {
        CodegenError::io(
            "atomically install fresh evidence vault without replacement",
            target,
            source,
        )
    })
}

#[cfg(windows)]
fn verify_staged_plan(staging: &Path, plan: &AcquisitionPlan) -> Result<()> {
    let expected_staging = fs::canonicalize(staging).map_err(|source| {
        CodegenError::io("canonicalize staged evidence vault root", staging, source)
    })?;
    let approved_staging = ApprovedExternalRoot::capture(
        &expected_staging,
        "staged evidence vault root",
        |resolved| {
            if !is_same_path(resolved, &expected_staging) {
                return Err(CodegenError::new(format!(
                    "staged evidence vault root `{}` resolved to a different canonical directory `{}`",
                    expected_staging.display(),
                    resolved.display()
                )));
            }
            if is_same_or_below(&plan.repo_root, resolved) {
                return Err(CodegenError::new(format!(
                    "staged evidence vault root `{}` resolved inside repository `{}`",
                    resolved.display(),
                    plan.repo_root.display()
                )));
            }
            reject_sensitive_path(resolved, "staged evidence vault root")
        },
    )?;
    let mut expected_paths = BTreeSet::new();
    expected_paths.insert(EVIDENCE_VAULT_CATALOG_FILE.to_owned());
    for entry in &plan.catalog.entries {
        expected_paths.insert(entry.content_path.clone());
        let path = portable_join(staging, &entry.content_path, "staged vault content")?;
        let observed = hash_regular_file(
            &plan.repo_root,
            &path,
            "staged vault content",
            Some(&approved_staging),
        )?;
        if observed != (entry.sha256.clone(), entry.size_bytes) {
            return Err(CodegenError::new(format!(
                "staged content verification failed for `{}`",
                entry.evidence_id
            )));
        }
    }
    let actual_paths = collect_regular_tree_paths(staging)?;
    if actual_paths != expected_paths {
        let extra: Vec<&str> = actual_paths
            .difference(&expected_paths)
            .map(String::as_str)
            .collect();
        let missing: Vec<&str> = expected_paths
            .difference(&actual_paths)
            .map(String::as_str)
            .collect();
        return Err(CodegenError::new(format!(
            "staged vault file bijection failed; extra=[{}] missing=[{}]",
            extra.join(", "),
            missing.join(", ")
        )));
    }
    let catalog_path = staging.join(EVIDENCE_VAULT_CATALOG_FILE);
    let actual_catalog = read_external_bytes_under(
        &approved_staging,
        &catalog_path,
        "staged evidence vault catalog",
    )?;
    if actual_catalog != plan.catalog_bytes {
        return Err(CodegenError::new(
            "staged evidence vault catalog bytes drifted",
        ));
    }
    let parsed = parse_strict(&actual_catalog, &catalog_path)?;
    if actual_catalog != canonical_bytes(&parsed) {
        return Err(CodegenError::new(format!(
            "staged evidence vault catalog `{}` is not canonical `{CANONICALIZATION_ID}` JSON",
            catalog_path.display()
        )));
    }
    let parsed: EvidenceVaultCatalog =
        serde_json::from_value(parsed.into_serde()).map_err(|source| {
            CodegenError::with_source(
                format!(
                    "closed-structure load of staged evidence vault catalog `{}` failed",
                    catalog_path.display()
                ),
                source,
            )
        })?;
    validate_emitted_catalog(&parsed)
}

#[cfg(windows)]
fn copy_verified_regular_file(
    repo_root: &Path,
    content: &VerifiedContent,
    destination: &Path,
) -> Result<()> {
    let expected_source = content.source_path.clone();
    let approved_source =
        ApprovedExternalFile::capture(&content.source_path, "vault source asset", |resolved| {
            if !is_same_path(resolved, &expected_source) {
                return Err(CodegenError::new(format!(
                    "vault source asset `{}` resolved to a different canonical file `{}`",
                    expected_source.display(),
                    resolved.display()
                )));
            }
            if is_same_or_below(repo_root, resolved) {
                return Err(CodegenError::new(format!(
                    "vault source asset `{}` resolved inside repository `{}`",
                    resolved.display(),
                    repo_root.display()
                )));
            }
            reject_sensitive_path(resolved, "vault source asset")
        })?;
    let source_bytes = read_external_bytes_bound(approved_source, "vault source asset")?;
    let size = source_bytes.len() as u64;
    let hash = sha256_hex(&source_bytes);
    if size != content.size_bytes || hash != content.sha256 {
        return Err(CodegenError::new(format!(
            "vault source asset `{}` changed after planning: expected {} bytes/{} observed {size} bytes/{hash}",
            content.source_path.display(),
            content.size_bytes,
            content.sha256
        )));
    }
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|source| CodegenError::io("create vault content file", destination, source))?;
    output.write_all(&source_bytes).map_err(|source| {
        CodegenError::io("write content-addressed vault file", destination, source)
    })?;
    output.sync_all().map_err(|source| {
        CodegenError::io("sync content-addressed vault file", destination, source)
    })?;
    Ok(())
}

fn hash_regular_file(
    repo_root: &Path,
    path: &Path,
    label: &str,
    approved_root: Option<&ApprovedExternalRoot>,
) -> Result<(String, u64)> {
    let validate = |resolved: &Path| {
        if !is_same_path(resolved, path) {
            return Err(CodegenError::new(format!(
                "{label} `{}` resolved to a different canonical file `{}`",
                path.display(),
                resolved.display()
            )));
        }
        if is_same_or_below(repo_root, resolved) {
            return Err(CodegenError::new(format!(
                "{label} `{}` resolved inside repository `{}`",
                resolved.display(),
                repo_root.display()
            )));
        }
        reject_sensitive_path(resolved, label)
    };
    let bytes = if let Some(root) = approved_root {
        root.revalidate(label)?;
        let approved = ApprovedExternalFile::capture(path, label, |resolved| {
            validate(resolved)?;
            if !is_same_or_below(root.path(), resolved) {
                return Err(CodegenError::new(format!(
                    "{label} `{}` is outside approved root `{}`",
                    resolved.display(),
                    root.path().display()
                )));
            }
            Ok(())
        })?;
        let read_result = read_external_bytes_bound(approved, label);
        root.revalidate(label)?;
        read_result?
    } else {
        let approved = ApprovedExternalFile::capture(path, label, validate)?;
        read_external_bytes_bound(approved, label)?
    };
    Ok((sha256_hex(&bytes), bytes.len() as u64))
}

fn encode_digest(digest: impl AsRef<[u8]>) -> String {
    let bytes = digest.as_ref();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn sha256_hex(bytes: &[u8]) -> String {
    encode_digest(Sha256::digest(bytes))
}

fn content_addressed_path(hash: &str) -> String {
    format!("{CONTENT_ADDRESS_PREFIX}{}/{}", &hash[..2], hash)
}

fn canonical_external_regular_file(repo_root: &Path, path: &Path, label: &str) -> Result<PathBuf> {
    require_absolute_normalized(path, label)?;
    reject_symlink_ancestors(path, label)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| CodegenError::io(&format!("inspect {label}"), path, source))?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_file() {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be a real regular file",
            path.display()
        )));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label}"), path, source))?;
    if is_same_or_below(repo_root, &canonical) {
        return Err(CodegenError::new(format!(
            "{label} `{}` must remain outside repository `{}`",
            path.display(),
            repo_root.display()
        )));
    }
    reject_sensitive_path(&canonical, label)?;
    Ok(canonical)
}

fn canonical_source_asset_file(
    repo_root: &Path,
    path: &Path,
    asset_identity: &str,
) -> Result<PathBuf> {
    let label = format!("source-map path for `{asset_identity}`");
    let canonical = canonical_external_regular_file(repo_root, path, &label)?;
    reject_sensitive_path(&canonical, &label)?;
    Ok(canonical)
}

fn validate_fresh_external_vault_root(repo_root: &Path, target: &Path) -> Result<PathBuf> {
    require_absolute_normalized(target, "vault root")?;
    if target.exists() {
        return Err(CodegenError::new(format!(
            "vault root `{}` already exists; refusing to overwrite",
            target.display()
        )));
    }
    if is_same_or_below(repo_root, target) {
        return Err(CodegenError::new(format!(
            "vault root `{}` must remain outside repository `{}`",
            target.display(),
            repo_root.display()
        )));
    }
    reject_symlink_ancestors(target, "vault root")?;
    reject_sensitive_path(target, "vault root")?;
    let parent = target.parent().ok_or_else(|| {
        CodegenError::new(format!("vault root `{}` has no parent", target.display()))
    })?;
    let canonical_parent = canonical_real_directory(parent, "vault parent")?;
    if is_same_or_below(repo_root, &canonical_parent) {
        return Err(CodegenError::new(format!(
            "vault root `{}` resolves beneath repository `{}`",
            target.display(),
            repo_root.display()
        )));
    }
    reject_sensitive_path(&canonical_parent, "vault parent")?;
    Ok(canonical_parent.join(
        target
            .file_name()
            .ok_or_else(|| CodegenError::new("vault root has no final component"))?,
    ))
}

fn require_absolute_normalized(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be an explicit absolute, lexically normalized OS path",
            path.display()
        )));
    }
    Ok(())
}

fn reject_sensitive_path(path: &Path, label: &str) -> Result<()> {
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
    let has_sensitive_component = components.iter().any(|component| {
        let normalized = component.replace([' ', '_'], "-");
        normalized.contains("taxpayer")
            || normalized.contains("tax-payer")
            || normalized.contains("final-copy")
            || normalized.contains("finalcopy")
            || normalized.starts_with("save-")
            || normalized.ends_with("-save")
            || normalized.contains("-save-")
            || normalized.starts_with("savefile-")
            || normalized.ends_with("-savefile")
            || normalized.contains("-savefile-")
            || normalized.starts_with("profile-")
            || normalized.ends_with("-profile")
            || normalized.contains("-profile-")
            || matches!(
                normalized.as_str(),
                "save"
                    | "saved"
                    | "saves"
                    | "savefile"
                    | "savefiles"
                    | "profile"
                    | "profiles"
                    | "return-payload"
                    | "submission-payload"
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
                    | "secret"
                    | "secrets"
            )
    });
    let sensitive = has_pair("ebirforms", "savefile")
        || has_pair("ebirforms", "profile")
        || has_sensitive_component
        || components
            .iter()
            .any(|component| component == "group.dev.goldcoders.bir")
        || components
            .last()
            .is_some_and(|component| component == "bir_data.db")
        || components.iter().any(|component| {
            matches!(
                component.as_str(),
                "taxpayer-data" | "taxpayer_data" | "live-taxpayer-data"
            )
        });
    if sensitive {
        return Err(CodegenError::new(format!(
            "{label} `{}` is beneath a credential, secret, taxpayer/save, or live-database path",
            path.display()
        )));
    }
    Ok(())
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

fn canonical_real_directory(path: &Path, label: &str) -> Result<PathBuf> {
    reject_symlink_ancestors(path, label)?;
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

#[cfg(test)]
fn load_canonical_json<T>(path: &Path, label: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let bytes = read_external_file_bytes(path, label)?;
    let parsed = parse_strict(&bytes, path)?;
    if bytes != canonical_bytes(&parsed) {
        return Err(CodegenError::new(format!(
            "{label} `{}` is not canonical `{CANONICALIZATION_ID}` JSON",
            path.display()
        )));
    }
    serde_json::from_value(parsed.into_serde()).map_err(|source| {
        CodegenError::with_source(
            format!(
                "closed-structure load of {label} `{}` failed",
                path.display()
            ),
            source,
        )
    })
}

fn load_external_canonical_json<T>(
    repo_root: &Path,
    path: &Path,
    label: &str,
) -> Result<(PathBuf, T)>
where
    T: for<'de> Deserialize<'de>,
{
    require_absolute_normalized(path, label)?;
    let expected = canonical_external_regular_file(repo_root, path, label)?;
    let approved = ApprovedExternalFile::capture(path, label, |resolved| {
        if !is_same_path(resolved, &expected) {
            return Err(CodegenError::new(format!(
                "{label} `{}` resolved to a different canonical file `{}`",
                expected.display(),
                resolved.display()
            )));
        }
        if is_same_or_below(repo_root, resolved) {
            return Err(CodegenError::new(format!(
                "{label} `{}` must remain outside repository `{}`",
                resolved.display(),
                repo_root.display()
            )));
        }
        reject_sensitive_path(resolved, label)
    })?;
    let canonical_path = approved.path().to_path_buf();
    let bytes = read_external_bytes_bound(approved, label)?;
    let parsed = parse_strict(&bytes, &canonical_path)?;
    if bytes != canonical_bytes(&parsed) {
        return Err(CodegenError::new(format!(
            "{label} `{}` is not canonical `{CANONICALIZATION_ID}` JSON",
            canonical_path.display()
        )));
    }
    let value = serde_json::from_value(parsed.into_serde()).map_err(|source| {
        CodegenError::with_source(
            format!(
                "closed-structure load of {label} `{}` failed",
                canonical_path.display()
            ),
            source,
        )
    })?;
    Ok((canonical_path, value))
}

fn canonical_serialize(value: &impl Serialize, label: &str) -> Result<Vec<u8>> {
    let ordinary = serde_json::to_vec(value)
        .map_err(|source| CodegenError::with_source(format!("serialize {label}"), source))?;
    let parsed = parse_strict(&ordinary, Path::new(label))?;
    Ok(canonical_bytes(&parsed))
}

#[cfg(test)]
fn read_external_file_bytes(path: &Path, label: &str) -> Result<Vec<u8>> {
    let expected = fs::canonicalize(path)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label}"), path, source))?;
    let approved = ApprovedExternalFile::capture(path, label, |resolved| {
        if !is_same_path(resolved, &expected) {
            return Err(CodegenError::new(format!(
                "{label} `{}` resolved to a different canonical file `{}`",
                expected.display(),
                resolved.display()
            )));
        }
        Ok(())
    })?;
    read_external_bytes_bound(approved, label)
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<&'a Vec<Value>> {
    object
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
            "{label} must be non-empty, trimmed, bounded, control-free text"
        )));
    }
    reject_machine_locator(value, label)?;
    reject_sensitive_text(value, label)
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
        Value::String(value) => reject_machine_locator(value, label)?,
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
            "{label} contains a machine-local path, which must not enter the vault catalog"
        )));
    }
    Ok(())
}

#[cfg(any(windows, test))]
fn collect_regular_tree_paths(root: &Path) -> Result<BTreeSet<String>> {
    fn visit(root: &Path, directory: &Path, paths: &mut BTreeSet<String>) -> Result<()> {
        let entries = fs::read_dir(directory)
            .map_err(|source| CodegenError::io("read staged vault directory", directory, source))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|source| {
                CodegenError::io("read staged vault directory entry", directory, source)
            })?;
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|source| CodegenError::io("inspect staged vault entry", &path, source))?;
            if is_symlink_or_reparse_point(&metadata) {
                return Err(CodegenError::new(format!(
                    "staged vault contains symlink/reparse point `{}`",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                visit(root, &path, paths)?;
            } else if metadata.is_file() {
                paths.insert(normalized_relative_path(root, &path)?);
            } else {
                return Err(CodegenError::new(format!(
                    "staged vault contains non-regular entry `{}`",
                    path.display()
                )));
            }
        }
        Ok(())
    }

    let mut paths = BTreeSet::new();
    visit(root, root, &mut paths)?;
    Ok(paths)
}

#[cfg(any(windows, test))]
fn normalized_relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        CodegenError::new(format!(
            "staged vault path `{}` escapes `{}`",
            path.display(),
            root.display()
        ))
    })?;
    let mut output = String::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(CodegenError::new(format!(
                "staged vault path `{}` is not normalized",
                path.display()
            )));
        };
        let component = component.to_str().ok_or_else(|| {
            CodegenError::new(format!(
                "staged vault path `{}` is not valid UTF-8",
                path.display()
            ))
        })?;
        if !output.is_empty() {
            output.push('/');
        }
        output.push_str(component);
    }
    Ok(output)
}

#[cfg(windows)]
fn sync_directory(path: &Path) -> Result<()> {
    match File::open(path).and_then(|directory| directory.sync_all()) {
        Ok(()) => Ok(()),
        Err(source) if cfg!(windows) => {
            let _ = source;
            Ok(())
        }
        Err(source) => Err(CodegenError::io(
            "sync vault output directory",
            path,
            source,
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::{Value, json};

    #[cfg(windows)]
    use super::publish_directory_no_replace;
    use super::{
        AcquireEvidenceVaultOptions, AcquisitionPlan, EVIDENCE_VAULT_CAPTURE_METADATA_FORMAT,
        EVIDENCE_VAULT_CATALOG_FILE, EVIDENCE_VAULT_SOURCE_MAP_FORMAT,
        EXPECTED_V1_FORM_MANIFEST_COUNT, EvidenceVaultCaptureMetadata, EvidenceVaultSourceMap,
        EvidenceVaultSourceMapEntry, STAGING_MARKER, VaultAssetDisposition,
        VerifyEvidenceVaultSourceMapOptions, acquire_evidence_vault, build_acquisition_plan,
        canonical_external_regular_file, canonical_serialize, collect_regular_tree_paths,
        install_plan_atomically, load_canonical_json, load_declared_manifest_assets, parse_strict,
        sha256_hex, validate_capture_metadata, validate_source_map_header,
        verify_evidence_vault_source_map,
    };
    use crate::evidence::{EvidenceCaptureOperatingSystem, EvidenceCaptureProvenance};
    use crate::files::ReadScope;
    use crate::path::canonical_repo_root;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
    const OFFICIAL_BYTES: &[u8] = b"synthetic official package fixture bytes";

    struct TestRoot {
        path: PathBuf,
    }

    impl TestRoot {
        fn new(label: &str) -> Self {
            let path = crate::test_temp_dir().join(format!(
                "bir-vault-acquisition-{label}-{}-{}",
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

    struct Fixture {
        _root: TestRoot,
        repo_root: PathBuf,
        source_path: PathBuf,
        source_map_path: PathBuf,
        capture_metadata_path: PathBuf,
        source_map: EvidenceVaultSourceMap,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let root = TestRoot::new(label);
            let repo_root = root.path.join("repo");
            let forms_root = repo_root.join("rules/forms");
            let input_root = root.path.join("official-input");
            fs::create_dir_all(&forms_root).expect("create fixture forms root");
            fs::create_dir(&input_root).expect("create fixture input root");
            let source_path = input_root.join("official-package.bin");
            fs::write(&source_path, OFFICIAL_BYTES).expect("write fixture official bytes");
            let hash = sha256_hex(OFFICIAL_BYTES);

            let mut source_entries = Vec::new();
            for ordinal in 1..=EXPECTED_V1_FORM_MANIFEST_COUNT {
                let form_id = format!("form-{ordinal:03}");
                let asset_id = format!("package-{ordinal:03}");
                let form_root = forms_root.join(&form_id);
                fs::create_dir(&form_root).expect("create fixture form root");
                write_canonical_json(
                    &form_root.join("manifest.json"),
                    &json!({
                        "form_id": form_id,
                        "official_assets": [{
                            "asset_id": asset_id,
                            "kind": "official-package-executable",
                            "path": "C:\\machine-local\\must-never-be-operative.exe",
                            "sha256": hash,
                            "size": OFFICIAL_BYTES.len()
                        }]
                    }),
                );
                source_entries.push(EvidenceVaultSourceMapEntry {
                    form_id,
                    asset_id,
                    source_path: source_path.to_string_lossy().into_owned(),
                });
            }
            let source_map = EvidenceVaultSourceMap {
                format: EVIDENCE_VAULT_SOURCE_MAP_FORMAT.to_owned(),
                canonicalization: crate::json::CANONICALIZATION_ID.to_owned(),
                entries: source_entries,
            };
            let source_map_path = root.path.join("source-map.json");
            write_canonical_json(&source_map_path, &source_map);
            let verified_source_map = verify_evidence_vault_source_map(
                &VerifyEvidenceVaultSourceMapOptions::external_workspace(
                    &repo_root,
                    &source_map_path,
                ),
            )
            .expect("verify fixture source map before binding capture metadata");

            let capture_metadata = EvidenceVaultCaptureMetadata {
                format: EVIDENCE_VAULT_CAPTURE_METADATA_FORMAT.to_owned(),
                canonicalization: crate::json::CANONICALIZATION_ID.to_owned(),
                capture_session_id: "static-official-package-capture".to_owned(),
                source_map_sha256: verified_source_map.source_map_sha256,
                source_verification_sha256: verified_source_map.verification_sha256,
                capture_provenance: EvidenceCaptureProvenance {
                    tool_commit: "a".repeat(40),
                    command_argv: vec![
                        "bir-rules-codegen".to_owned(),
                        "verify-evidence-vault-source-map".to_owned(),
                        "--source-map".to_owned(),
                        "../source-map.json".to_owned(),
                    ],
                    capture_tool_version: "bir-rules-codegen 0.1.0".to_owned(),
                    operating_system: EvidenceCaptureOperatingSystem::Windows,
                    windows_version: "Windows 11 23H2".to_owned(),
                    official_app_version: "7.9.6.0".to_owned(),
                    started_at_utc: "2026-07-26T01:00:00Z".to_owned(),
                    finished_at_utc: "2026-07-26T01:05:00Z".to_owned(),
                },
            };
            let capture_metadata_path = root.path.join("capture-metadata.json");
            write_canonical_json(&capture_metadata_path, &capture_metadata);

            Self {
                _root: root,
                repo_root,
                source_path,
                source_map_path,
                capture_metadata_path,
                source_map,
            }
        }

        fn vault_root(&self, name: &str) -> PathBuf {
            self._root.path.join(name)
        }

        fn options(&self, vault_name: &str, dry_run: bool) -> AcquireEvidenceVaultOptions {
            let mut options = AcquireEvidenceVaultOptions::external_workspace(
                &self.repo_root,
                &self.source_map_path,
                &self.capture_metadata_path,
                self.vault_root(vault_name),
            );
            options.dry_run = dry_run;
            options
        }

        fn write_source_map(&self) {
            write_canonical_json(&self.source_map_path, &self.source_map);
        }

        fn rebind_capture_metadata_to_current_source_map(&self) {
            let verification = verify_evidence_vault_source_map(
                &VerifyEvidenceVaultSourceMapOptions::external_workspace(
                    &self.repo_root,
                    &self.source_map_path,
                ),
            )
            .expect("verify current fixture source map");
            let mut metadata: EvidenceVaultCaptureMetadata =
                load_canonical_json(&self.capture_metadata_path, "test capture metadata")
                    .expect("load fixture capture metadata");
            metadata.source_map_sha256 = verification.source_map_sha256;
            metadata.source_verification_sha256 = verification.verification_sha256;
            write_canonical_json(&self.capture_metadata_path, &metadata);
        }

        fn mutate_manifest_asset(
            &self,
            form_id: &str,
            mutate: impl FnOnce(&mut serde_json::Map<String, Value>),
        ) {
            let path = self
                .repo_root
                .join("rules/forms")
                .join(form_id)
                .join("manifest.json");
            let bytes = fs::read(&path).expect("read manifest for mutation");
            let mut value = parse_strict(&bytes, &path)
                .expect("strict fixture manifest")
                .into_serde();
            let asset = value
                .get_mut("official_assets")
                .and_then(Value::as_array_mut)
                .and_then(|assets| assets.first_mut())
                .and_then(Value::as_object_mut)
                .expect("fixture manifest asset");
            mutate(asset);
            write_canonical_json(&path, &value);
        }
    }

    fn write_canonical_json(path: &Path, value: &impl serde::Serialize) {
        fs::write(
            path,
            canonical_serialize(value, "test fixture JSON").expect("canonical test JSON"),
        )
        .expect("write canonical test JSON");
    }

    fn prepare_plan(fixture: &Fixture) -> AcquisitionPlan {
        let repo_root = canonical_repo_root(&fixture.repo_root).expect("canonical fixture repo");
        let source_map_path = canonical_external_regular_file(
            &repo_root,
            &fixture.source_map_path,
            "test source map",
        )
        .expect("external source map");
        let metadata_path = canonical_external_regular_file(
            &repo_root,
            &fixture.capture_metadata_path,
            "test capture metadata",
        )
        .expect("external capture metadata");
        let source_map: EvidenceVaultSourceMap =
            load_canonical_json(&source_map_path, "test source map").expect("load source map");
        validate_source_map_header(&source_map).expect("valid source-map header");
        let metadata: EvidenceVaultCaptureMetadata =
            load_canonical_json(&metadata_path, "test metadata").expect("load metadata");
        validate_capture_metadata(&metadata).expect("valid capture metadata");
        let (count, assets) = load_declared_manifest_assets(&repo_root, ReadScope::External)
            .expect("load fixture manifests");
        build_acquisition_plan(
            &repo_root,
            &source_map_path,
            count,
            assets,
            source_map,
            metadata,
        )
        .expect("build fixture acquisition plan")
    }

    #[test]
    fn exact_43_form_manifest_fixture_plans_without_writes() {
        let fixture = Fixture::new("exact-43");
        let options = fixture.options("dry-vault", true);
        let report = acquire_evidence_vault(&options).expect("plan exact fixture");
        assert_eq!(report.manifest_count, EXPECTED_V1_FORM_MANIFEST_COUNT);
        assert_eq!(report.declared_asset_count, EXPECTED_V1_FORM_MANIFEST_COUNT);
        assert_eq!(report.mapped_asset_count, EXPECTED_V1_FORM_MANIFEST_COUNT);
        assert_eq!(report.verified_source_file_count, 1);
        assert_eq!(report.unique_content_count, 1);
        assert_eq!(
            report.deduplicated_asset_count,
            EXPECTED_V1_FORM_MANIFEST_COUNT - 1
        );
        assert!(report.gaps.is_empty());
        assert!(!report.written);
        assert!(!options.vault_root.exists(), "dry-run must write nothing");
    }

    #[test]
    fn capture_metadata_requires_the_exact_portable_source_verifier_argv() {
        let fixture = Fixture::new("exact-verifier-argv");
        let metadata: EvidenceVaultCaptureMetadata =
            load_canonical_json(&fixture.capture_metadata_path, "test capture metadata")
                .expect("load fixture metadata");
        validate_capture_metadata(&metadata).expect("exact verifier argv must pass");

        for bad_argv in [
            vec!["unrelated-command".to_owned()],
            vec![
                "bir-rules-codegen".to_owned(),
                "verify-evidence-vault-source-map".to_owned(),
                "--source-map".to_owned(),
                "evidence/source-map.json".to_owned(),
            ],
            vec![
                "bir-rules-codegen".to_owned(),
                "verify-evidence-vault-source-map".to_owned(),
                "--source-map".to_owned(),
                r"..\evidence\source-map.json".to_owned(),
            ],
        ] {
            let mut candidate = metadata.clone();
            candidate.capture_provenance.command_argv = bad_argv;
            validate_capture_metadata(&candidate)
                .expect_err("non-exact or non-portable verifier argv must fail");
        }
    }

    #[test]
    fn acquisition_binds_recorded_verifier_locator_to_the_exact_source_map() {
        let fixture = Fixture::new("verifier-locator-binding");
        let other_root = fixture._root.path.join("other");
        fs::create_dir(&other_root).expect("create alternate source-map parent");
        let other_map = other_root.join("source-map.json");
        write_canonical_json(&other_map, &fixture.source_map);

        let mut metadata: EvidenceVaultCaptureMetadata =
            load_canonical_json(&fixture.capture_metadata_path, "test capture metadata")
                .expect("load fixture metadata");
        metadata.capture_provenance.command_argv[3] = "../other/source-map.json".to_owned();
        write_canonical_json(&fixture.capture_metadata_path, &metadata);

        let error = acquire_evidence_vault(&fixture.options("dry-vault", true))
            .expect_err("same bytes at a different locator must not satisfy provenance");
        assert!(
            error
                .to_string()
                .contains("does not resolve to the source map being acquired"),
            "{error}"
        );
    }

    #[test]
    fn capture_provenance_rejects_sensitive_text_channels() {
        let fixture = Fixture::new("sensitive-capture-provenance");
        let metadata: EvidenceVaultCaptureMetadata =
            load_canonical_json(&fixture.capture_metadata_path, "test capture metadata")
                .expect("load fixture metadata");

        let mutations: [fn(&mut EvidenceVaultCaptureMetadata); 4] = [
            |candidate: &mut EvidenceVaultCaptureMetadata| {
                candidate.capture_provenance.capture_tool_version = "password=hunter2".to_owned();
            },
            |candidate: &mut EvidenceVaultCaptureMetadata| {
                candidate.capture_provenance.windows_version = "reviewer@example.test".to_owned();
            },
            |candidate: &mut EvidenceVaultCaptureMetadata| {
                candidate.capture_provenance.official_app_version = "123-456-789".to_owned();
            },
            |candidate: &mut EvidenceVaultCaptureMetadata| {
                candidate.capture_provenance.command_argv[3] =
                    "../password=hunter2/source-map.json".to_owned();
            },
        ];
        for mutate in mutations {
            let mut candidate = metadata.clone();
            mutate(&mut candidate);
            let error = validate_capture_metadata(&candidate)
                .expect_err("sensitive capture provenance must fail");
            assert!(
                error.to_string().contains("credential")
                    || error.to_string().contains("taxpayer")
                    || error.to_string().contains("email"),
                "{error}"
            );
        }
    }

    #[test]
    fn source_map_verifier_is_no_write_and_binds_all_manifest_assets() {
        let fixture = Fixture::new("source-map-verifier");
        let report = verify_evidence_vault_source_map(
            &VerifyEvidenceVaultSourceMapOptions::external_workspace(
                &fixture.repo_root,
                &fixture.source_map_path,
            ),
        )
        .expect("verify complete source map");
        assert_eq!(report.manifest_count, EXPECTED_V1_FORM_MANIFEST_COUNT);
        assert_eq!(report.mapped_asset_count, EXPECTED_V1_FORM_MANIFEST_COUNT);
        assert_eq!(report.verified_source_file_count, 1);
        assert_eq!(report.unique_content_count, 1);
        assert_eq!(
            report.deduplicated_asset_count,
            EXPECTED_V1_FORM_MANIFEST_COUNT - 1
        );
        assert_eq!(report.source_map_sha256.len(), 64);
        assert_eq!(report.verification_sha256.len(), 64);
        let acquisition = acquire_evidence_vault(&fixture.options("dry-vault", true))
            .expect("build capture-bound dry-run report");
        assert_eq!(acquisition.source_map_sha256, report.source_map_sha256);
        assert_eq!(
            acquisition.source_verification_sha256,
            report.verification_sha256
        );
        assert!(acquisition.catalog.entries.iter().all(|entry| {
            entry.source_map_sha256 == report.source_map_sha256
                && entry.source_verification_sha256 == report.verification_sha256
        }));
        assert!(
            !fixture.vault_root("verifier-must-not-write").exists(),
            "source-map verification must not create a vault or metadata"
        );
    }

    #[test]
    fn hash_or_size_mismatch_fails_before_output() {
        let fixture = Fixture::new("hash-mismatch");
        fs::write(&fixture.source_path, b"tampered fixture bytes").expect("tamper fixture source");
        let options = fixture.options("vault", true);
        let error = acquire_evidence_vault(&options).expect_err("hash mismatch must fail");
        assert!(
            error.to_string().contains("do not match manifest identity"),
            "{error}"
        );
        assert!(!options.vault_root.exists());
    }

    #[test]
    fn source_map_rejects_missing_and_extra_entries() {
        let mut missing_fixture = Fixture::new("missing-map");
        missing_fixture.source_map.entries.remove(0);
        missing_fixture.write_source_map();
        let error = acquire_evidence_vault(&missing_fixture.options("vault", true))
            .expect_err("missing source-map entry must fail");
        assert!(error.to_string().contains("missing=[form-001#package-001]"));

        let mut extra_fixture = Fixture::new("extra-map");
        extra_fixture
            .source_map
            .entries
            .push(EvidenceVaultSourceMapEntry {
                form_id: "form-999".to_owned(),
                asset_id: "package-999".to_owned(),
                source_path: extra_fixture.source_path.to_string_lossy().into_owned(),
            });
        extra_fixture.write_source_map();
        let error = acquire_evidence_vault(&extra_fixture.options("vault", true))
            .expect_err("extra source-map entry must fail");
        assert!(error.to_string().contains("extra=[form-999#package-999]"));
    }

    #[cfg(windows)]
    #[test]
    fn duplicate_bytes_are_deduplicated_by_hash_and_size() {
        let fixture = Fixture::new("dedup");
        let options = fixture.options("vault", false);
        let report = acquire_evidence_vault(&options).expect("write deduplicated vault");
        assert!(report.written);
        assert_eq!(report.unique_content_count, 1);
        assert_eq!(
            report.deduplicated_asset_count,
            EXPECTED_V1_FORM_MANIFEST_COUNT - 1
        );
        let paths = collect_regular_tree_paths(&options.vault_root).expect("inspect written vault");
        assert_eq!(paths.len(), 2, "one catalog plus one unique content file");
        let entry = &report.catalog.entries[0];
        assert_eq!(
            fs::read(
                options
                    .vault_root
                    .join(entry.content_path.replace('/', "\\"))
            )
            .or_else(|_| fs::read(options.vault_root.join(&entry.content_path)))
            .expect("read content-addressed bytes"),
            OFFICIAL_BYTES
        );
    }

    #[test]
    fn taxpayer_shaped_kinds_are_gaps_and_cannot_enter_the_map() {
        let mut fixture = Fixture::new("metadata-gap");
        fixture.mutate_manifest_asset("form-001", |asset| {
            asset.insert(
                "kind".to_owned(),
                Value::String("dummy-profile-encrypted-final-copy".to_owned()),
            );
        });
        let forbidden_entry = fixture.source_map.entries.remove(0);
        fixture.write_source_map();
        fixture.rebind_capture_metadata_to_current_source_map();
        let report = acquire_evidence_vault(&fixture.options("vault", true))
            .expect("metadata-only declaration should remain an explicit gap");
        assert_eq!(report.gaps.len(), 1);
        assert_eq!(report.gaps[0].form_id, "form-001");
        assert_eq!(report.gaps[0].asset_id, "package-001");
        assert_eq!(report.gaps[0].kind, "dummy-profile-encrypted-final-copy");
        assert_eq!(report.gaps[0].sha256, sha256_hex(OFFICIAL_BYTES));
        assert_eq!(report.gaps[0].size_bytes, OFFICIAL_BYTES.len() as u64);
        assert_eq!(
            report.gaps[0].disposition,
            VaultAssetDisposition::MetadataOnlyTaxpayerPayload
        );

        fixture.source_map.entries.push(forbidden_entry);
        fixture.write_source_map();
        let error = acquire_evidence_vault(&fixture.options("vault", true))
            .expect_err("metadata-only asset map entry must fail");
        assert!(
            error.to_string().contains("must not supply bytes"),
            "{error}"
        );
    }

    #[test]
    fn zero_size_provenance_is_an_explicit_gap_not_a_file() {
        let mut fixture = Fixture::new("zero-gap");
        fixture.mutate_manifest_asset("form-001", |asset| {
            asset.insert(
                "kind".to_owned(),
                Value::String("prior-reviewed-repository-provenance".to_owned()),
            );
            asset.insert("size".to_owned(), Value::from(0_u64));
        });
        fixture.source_map.entries.remove(0);
        fixture.write_source_map();
        fixture.rebind_capture_metadata_to_current_source_map();
        let report =
            acquire_evidence_vault(&fixture.options("vault", true)).expect("plan zero-size gap");
        assert_eq!(report.gaps.len(), 1);
        assert_eq!(
            report.gaps[0].disposition,
            VaultAssetDisposition::ZeroSizeProvenance
        );
        assert_eq!(report.gaps[0].size_bytes, 0);
        assert_eq!(report.catalog.entries.len(), 1);
    }

    #[test]
    fn source_map_verification_rejects_sensitive_paths_before_reading_bytes() {
        let sensitive_components = [
            ".ssh",
            ".aws",
            ".azure",
            ".gnupg",
            ".kube",
            ".docker",
            "keychain",
            "credentials",
            "secrets",
            "taxpayer-data",
            "return-savefile",
        ];
        for component in sensitive_components {
            let mut fixture = Fixture::new("sensitive-source");
            let sensitive_root = fixture._root.path.join(component);
            fs::create_dir_all(&sensitive_root).expect("create synthetic sensitive root");
            let sensitive_source = sensitive_root.join("official-package.bin");
            // Matching bytes ensure the sensitive-path callback, rather than a
            // later hash mismatch, is the reason verification fails.
            fs::write(&sensitive_source, OFFICIAL_BYTES).expect("write synthetic sensitive source");
            let sensitive_path = sensitive_source.to_string_lossy().into_owned();
            for entry in &mut fixture.source_map.entries {
                entry.source_path = sensitive_path.clone();
            }
            fixture.write_source_map();

            let error = verify_evidence_vault_source_map(
                &VerifyEvidenceVaultSourceMapOptions::external_workspace(
                    &fixture.repo_root,
                    &fixture.source_map_path,
                ),
            )
            .expect_err("sensitive source-map path must fail before byte verification");
            assert!(
                error
                    .to_string()
                    .contains("credential, secret, taxpayer/save, or live-database path"),
                "{component}: {error}"
            );
        }
    }

    #[test]
    fn source_map_verification_rejects_hard_link_aliases_before_reading_bytes() {
        let mut fixture = Fixture::new("hard-link-alias");
        let alias = fixture
            ._root
            .path
            .join("official-input")
            .join("official-package-alias.bin");
        fs::hard_link(&fixture.source_path, &alias).expect("create source hard-link alias");
        let alias_path = alias.to_string_lossy().into_owned();
        for entry in &mut fixture.source_map.entries {
            entry.source_path = alias_path.clone();
        }
        fixture.write_source_map();

        let error = verify_evidence_vault_source_map(
            &VerifyEvidenceVaultSourceMapOptions::external_workspace(
                &fixture.repo_root,
                &fixture.source_map_path,
            ),
        )
        .expect_err("hard-linked source-map path must fail before byte verification");
        assert!(
            error.to_string().contains("hard links")
                && error.to_string().contains("aliased external inputs"),
            "{error}"
        );
    }

    #[test]
    fn symlink_or_reparse_source_is_rejected() {
        let mut fixture = Fixture::new("symlink");
        let link = fixture._root.path.join("official-input-link.bin");
        match create_file_symlink(&fixture.source_path, &link) {
            Ok(()) => {}
            Err(source)
                if source.kind() == io::ErrorKind::PermissionDenied
                    || source.kind() == io::ErrorKind::Unsupported
                    || source.raw_os_error() == Some(1314) =>
            {
                return;
            }
            Err(source) => panic!("create fixture symlink: {source}"),
        }
        fixture.source_map.entries[0].source_path = link.to_string_lossy().into_owned();
        fixture.write_source_map();
        let error = acquire_evidence_vault(&fixture.options("vault", true))
            .expect_err("symlink/reparse input must fail");
        assert!(
            error.to_string().contains("symlink") || error.to_string().contains("reparse point"),
            "{error}"
        );
    }

    #[test]
    fn canonical_inputs_and_duplicate_keys_are_enforced() {
        let fixture = Fixture::new("canonical");
        fs::write(
            &fixture.source_map_path,
            serde_json::to_vec_pretty(&fixture.source_map).expect("pretty source map"),
        )
        .expect("write noncanonical source map");
        let error = acquire_evidence_vault(&fixture.options("vault", true))
            .expect_err("noncanonical source map must fail");
        assert!(error.to_string().contains("is not canonical"), "{error}");

        fixture.write_source_map();
        let canonical_metadata =
            fs::read_to_string(&fixture.capture_metadata_path).expect("read metadata");
        let duplicate_metadata = format!(
            "{{\"format\":\"{EVIDENCE_VAULT_CAPTURE_METADATA_FORMAT}\",{}",
            &canonical_metadata[1..]
        );
        fs::write(&fixture.capture_metadata_path, duplicate_metadata)
            .expect("write duplicate-key metadata");
        let error = acquire_evidence_vault(&fixture.options("vault", true))
            .expect_err("duplicate metadata key must fail");
        let details = format!("{error:?}");
        assert!(
            details.contains("duplicate object key `format`"),
            "{details}"
        );
    }

    #[test]
    fn emitted_catalog_contains_no_machine_paths() {
        let fixture = Fixture::new("no-machine-path");
        let report =
            acquire_evidence_vault(&fixture.options("vault", true)).expect("plan safe catalog");
        let bytes = report
            .canonical_catalog_bytes()
            .expect("canonical catalog bytes");
        let text = String::from_utf8(bytes).expect("catalog is UTF-8");
        assert!(!text.contains("source_path"));
        assert!(!text.contains("machine-local"));
        assert!(!text.contains("must-never-be-operative"));
        assert!(!text.contains("official-package.bin"));
        assert!(!text.contains(&fixture._root.path.to_string_lossy().replace('\\', "\\\\")));
    }

    #[cfg(windows)]
    #[test]
    fn failed_install_leaves_ambiguous_staging_for_manual_inspection() {
        let fixture = Fixture::new("fail-closed-residue");
        let plan = prepare_plan(&fixture);
        fs::remove_file(&fixture.source_path).expect("remove planned source before install");
        let target = fixture.vault_root("vault");
        let error =
            install_plan_atomically(&target, &plan).expect_err("copy failure must abort install");
        assert!(
            error.to_string().contains("vault source asset")
                || error.to_string().contains("source asset"),
            "{error}"
        );
        assert!(!target.exists());
        let leftovers: Vec<String> = fs::read_dir(&fixture._root.path)
            .expect("read staging parent")
            .map(|entry| {
                entry
                    .expect("read staging sibling")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|name| name.contains(STAGING_MARKER))
            .collect();
        assert!(
            error.to_string().contains("left at"),
            "failure must identify fail-closed residue: {error}"
        );
        assert_eq!(leftovers.len(), 1, "expected one untouched staging residue");
        assert!(
            fixture._root.path.join(&leftovers[0]).is_dir(),
            "residue must remain a real directory for explicit inspection"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn write_publication_is_windows_only_but_dry_run_remains_available() {
        let fixture = Fixture::new("non-windows-publication");
        acquire_evidence_vault(&fixture.options("dry-vault", true))
            .expect("cross-platform dry-run remains available");
        let target = fixture.vault_root("write-vault");
        let error = acquire_evidence_vault(&fixture.options("write-vault", false))
            .expect_err("non-Windows publication must fail closed");
        assert!(
            error.to_string().contains("supported only on Windows"),
            "{error}"
        );
        assert!(!target.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_directory_publication_is_atomic_no_replace() {
        let root = TestRoot::new("windows-no-replace");
        let occupied_target = root.path.join("occupied-vault");
        fs::create_dir(&occupied_target).expect("create occupied target");
        fs::write(occupied_target.join("sentinel.txt"), b"must survive")
            .expect("write occupied target sentinel");
        let refused_staging = root.path.join("refused-staging");
        fs::create_dir(&refused_staging).expect("create refused staging");
        fs::write(refused_staging.join("new.txt"), b"new").expect("write staged content");

        let error = publish_directory_no_replace(&refused_staging, &occupied_target)
            .expect_err("publication must not replace an existing directory");
        assert!(error.to_string().contains("without replacement"), "{error}");
        assert_eq!(
            fs::read(occupied_target.join("sentinel.txt")).expect("read target sentinel"),
            b"must survive"
        );
        assert!(refused_staging.join("new.txt").is_file());

        let fresh_target = root.path.join("fresh-vault");
        publish_directory_no_replace(&refused_staging, &fresh_target)
            .expect("successful publication must return success");
        assert!(!refused_staging.exists());
        assert_eq!(
            fs::read(fresh_target.join("new.txt")).expect("read published content"),
            b"new"
        );
    }

    #[test]
    fn catalog_bytes_bind_map_order_but_not_vault_location() {
        let mut fixture = Fixture::new("byte-stability");
        let first = acquire_evidence_vault(&fixture.options("dry-one", true))
            .expect("first dry-run")
            .canonical_catalog_bytes()
            .expect("first catalog bytes");
        fixture.source_map.entries.reverse();
        fixture.write_source_map();
        fixture.rebind_capture_metadata_to_current_source_map();
        let second = acquire_evidence_vault(&fixture.options("dry-two", true))
            .expect("second dry-run")
            .canonical_catalog_bytes()
            .expect("second catalog bytes");
        assert_ne!(
            first, second,
            "catalog must carry the exact source-map digest"
        );
        let third = acquire_evidence_vault(&fixture.options("dry-three", true))
            .expect("same-map dry-run at another vault location")
            .canonical_catalog_bytes()
            .expect("third catalog bytes");
        assert_eq!(second, third, "vault location must not enter catalog bytes");

        #[cfg(windows)]
        {
            let first_options = fixture.options("vault-one", false);
            let second_options = fixture.options("vault-two", false);
            acquire_evidence_vault(&first_options).expect("write first vault");
            acquire_evidence_vault(&second_options).expect("write second vault");
            assert_eq!(
                fs::read(first_options.vault_root.join(EVIDENCE_VAULT_CATALOG_FILE))
                    .expect("read first catalog"),
                fs::read(second_options.vault_root.join(EVIDENCE_VAULT_CATALOG_FILE))
                    .expect("read second catalog")
            );
        }
    }

    #[cfg(unix)]
    fn create_file_symlink(source: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(source, link)
    }

    #[cfg(windows)]
    fn create_file_symlink(source: &Path, link: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_file(source, link)
    }
}
