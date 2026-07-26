//! Fail-closed discovery of external sources for the static evidence vault.
//!
//! The tracked manifest locator remains evidence, not authority. This module
//! uses it only as one candidate location, followed by caller-supplied external
//! search roots. Candidate bytes become operative only after their exact
//! manifest SHA-256 and size have been verified. Discovery either returns a
//! complete canonical [`EvidenceVaultSourceMap`] or a structured unresolved
//! report; a partial source map is never constructed or written.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as StdError;
use std::fmt;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use same_file::Handle;
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::error::{CodegenError, Result as CodegenResult};
use crate::json::{CANONICALIZATION_ID, canonical_bytes, parse_strict};
use crate::path::{canonical_repo_root, is_same_or_below, is_symlink_or_reparse_point};
use crate::vault_acquisition::{
    EVIDENCE_VAULT_SOURCE_MAP_FORMAT, EXPECTED_V1_FORM_MANIFEST_COUNT, EvidenceVaultSourceMap,
    EvidenceVaultSourceMapEntry, VaultAssetDisposition, vault_asset_disposition,
};
use crate::verified_file::open_verified_regular_file;
#[cfg(windows)]
use crate::verified_file::stable_windows_link_count;

const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// Complete inputs for a no-write discovery or fresh external source-map emit.
#[derive(Clone, Debug)]
pub struct DiscoverEvidenceVaultSourcesOptions {
    pub repo_root: PathBuf,
    pub search_roots: Vec<PathBuf>,
    pub output_path: PathBuf,
    pub dry_run: bool,
}

impl DiscoverEvidenceVaultSourcesOptions {
    pub fn new(repo_root: impl Into<PathBuf>, output_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            search_roots: Vec::new(),
            output_path: output_path.into(),
            dry_run: false,
        }
    }
}

/// A complete source-discovery result. This type cannot represent a partial map.
#[derive(Debug, Serialize)]
pub struct DiscoverEvidenceVaultSourcesReport {
    pub manifest_count: usize,
    pub declared_asset_count: usize,
    pub acquirable_asset_count: usize,
    pub metadata_only_asset_count: usize,
    pub zero_size_asset_count: usize,
    pub unique_content_count: usize,
    pub deduplicated_asset_count: usize,
    pub verified_candidate_file_count: usize,
    pub rejected_candidates: Vec<EvidenceVaultRejectedCandidate>,
    pub source_map: EvidenceVaultSourceMap,
    pub source_map_sha256: String,
    pub output_path: PathBuf,
    pub written: bool,
}

impl DiscoverEvidenceVaultSourcesReport {
    pub fn canonical_source_map_bytes(&self) -> CodegenResult<Vec<u8>> {
        canonical_serialize(&self.source_map, "evidence vault source map")
    }
}

/// One exact acquirable manifest declaration that discovery could not resolve.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EvidenceVaultUnresolvedAsset {
    pub form_id: String,
    pub asset_id: String,
    pub kind: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub manifest_locator: String,
}

impl EvidenceVaultUnresolvedAsset {
    fn identity(&self) -> String {
        format!("{}#{}", self.form_id, self.asset_id)
    }
}

/// A path that discovery deliberately did not open or could not verify.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EvidenceVaultRejectedCandidate {
    pub path: String,
    pub reason: String,
}

/// Deterministic detail returned when even one acquirable asset is unresolved.
///
/// This report intentionally contains no `EvidenceVaultSourceMap`: resolved
/// paths do not become an operational artifact until every required identity
/// is resolved.
#[derive(Debug, Serialize)]
pub struct EvidenceVaultUnresolvedReport {
    pub manifest_count: usize,
    pub declared_asset_count: usize,
    pub acquirable_asset_count: usize,
    pub metadata_only_asset_count: usize,
    pub zero_size_asset_count: usize,
    pub resolved_asset_count: usize,
    pub unresolved_asset_count: usize,
    pub unresolved_unique_content_count: usize,
    pub verified_candidate_file_count: usize,
    pub searched_roots: Vec<String>,
    pub unresolved_assets: Vec<EvidenceVaultUnresolvedAsset>,
    pub rejected_candidates: Vec<EvidenceVaultRejectedCandidate>,
}

/// Discovery distinguishes an invalid trust-boundary input from an exhaustive
/// but incomplete search.
#[derive(Debug)]
pub enum EvidenceVaultSourceDiscoveryError {
    Boundary(CodegenError),
    Unresolved(EvidenceVaultUnresolvedReport),
}

impl EvidenceVaultSourceDiscoveryError {
    pub fn unresolved_report(&self) -> Option<&EvidenceVaultUnresolvedReport> {
        match self {
            Self::Unresolved(report) => Some(report),
            Self::Boundary(_) => None,
        }
    }
}

impl fmt::Display for EvidenceVaultSourceDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boundary(error) => fmt::Display::fmt(error, formatter),
            Self::Unresolved(report) => {
                let identities = report
                    .unresolved_assets
                    .iter()
                    .map(EvidenceVaultUnresolvedAsset::identity)
                    .collect::<Vec<_>>();
                write!(
                    formatter,
                    "evidence vault source discovery unresolved {} acquirable manifest assets across {} expected hash/size identities: [{}]",
                    report.unresolved_asset_count,
                    report.unresolved_unique_content_count,
                    identities.join(", ")
                )
            }
        }
    }
}

impl StdError for EvidenceVaultSourceDiscoveryError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Boundary(error) => Some(error),
            Self::Unresolved(_) => None,
        }
    }
}

impl From<CodegenError> for EvidenceVaultSourceDiscoveryError {
    fn from(error: CodegenError) -> Self {
        Self::Boundary(error)
    }
}

pub type EvidenceVaultSourceDiscoveryResult<T> =
    std::result::Result<T, EvidenceVaultSourceDiscoveryError>;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AssetKey {
    form_id: String,
    asset_id: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ContentKey {
    sha256: String,
    size_bytes: u64,
}

#[derive(Clone, Debug)]
struct ExpectedAsset {
    key: AssetKey,
    kind: String,
    content: ContentKey,
    manifest_locator: String,
}

#[derive(Debug)]
struct ManifestInventory {
    manifest_count: usize,
    declared_asset_count: usize,
    metadata_only_asset_count: usize,
    zero_size_asset_count: usize,
    acquirable_assets: Vec<ExpectedAsset>,
    allowed_locator_leaves: BTreeSet<String>,
    forbidden_locator_leaves: BTreeSet<String>,
    forbidden_absolute_locators: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct ObservedFile {
    path: PathBuf,
    content: ContentKey,
}

#[derive(Debug, Default)]
struct CandidateState {
    verified_by_path: BTreeMap<PathBuf, ObservedFile>,
    rejected: BTreeSet<EvidenceVaultRejectedCandidate>,
}

impl CandidateState {
    fn reject(&mut self, path: &Path, reason: impl Into<String>) {
        self.rejected.insert(EvidenceVaultRejectedCandidate {
            path: path.to_string_lossy().into_owned(),
            reason: reason.into(),
        });
    }
}

/// Discover every acquirable manifest asset, verify the candidate bytes, and
/// optionally publish one complete canonical source map outside the repository.
pub fn discover_evidence_vault_sources(
    options: &DiscoverEvidenceVaultSourcesOptions,
) -> EvidenceVaultSourceDiscoveryResult<DiscoverEvidenceVaultSourcesReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let inventory = load_manifest_inventory(&repo_root)?;
    let output_path = validate_fresh_external_output(&repo_root, &options.output_path)?;
    let search_roots = validate_search_roots(&repo_root, &options.search_roots)?;

    let groups = group_expected_assets(&inventory.acquirable_assets);
    let all_expected_content = groups.keys().cloned().collect::<BTreeSet<_>>();
    let all_expected_sizes = all_expected_content
        .iter()
        .map(|content| content.size_bytes)
        .collect::<BTreeSet<_>>();
    let mut candidate_state = CandidateState::default();
    let mut resolved = BTreeMap::<ContentKey, PathBuf>::new();

    discover_exact_manifest_candidates(
        &repo_root,
        &groups,
        &all_expected_sizes,
        &inventory.forbidden_locator_leaves,
        &inventory.forbidden_absolute_locators,
        &mut resolved,
        &mut candidate_state,
    )?;

    let resolved_content = resolved.keys().cloned().collect::<BTreeSet<_>>();
    let mut needed = all_expected_content
        .difference(&resolved_content)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !needed.is_empty() {
        discover_in_search_roots(
            &repo_root,
            &search_roots,
            &inventory.allowed_locator_leaves,
            &inventory.forbidden_locator_leaves,
            &inventory.forbidden_absolute_locators,
            &mut needed,
            &mut resolved,
            &mut candidate_state,
        )?;
    }

    if !needed.is_empty() {
        let unresolved_assets = inventory
            .acquirable_assets
            .iter()
            .filter(|asset| needed.contains(&asset.content))
            .map(|asset| EvidenceVaultUnresolvedAsset {
                form_id: asset.key.form_id.clone(),
                asset_id: asset.key.asset_id.clone(),
                kind: asset.kind.clone(),
                sha256: asset.content.sha256.clone(),
                size_bytes: asset.content.size_bytes,
                manifest_locator: asset.manifest_locator.clone(),
            })
            .collect::<Vec<_>>();
        let unresolved_asset_count = unresolved_assets.len();
        let searched_roots = search_roots
            .iter()
            .map(|path| path_to_utf8(path))
            .collect::<CodegenResult<Vec<_>>>()?;
        return Err(EvidenceVaultSourceDiscoveryError::Unresolved(
            EvidenceVaultUnresolvedReport {
                manifest_count: inventory.manifest_count,
                declared_asset_count: inventory.declared_asset_count,
                acquirable_asset_count: inventory.acquirable_assets.len(),
                metadata_only_asset_count: inventory.metadata_only_asset_count,
                zero_size_asset_count: inventory.zero_size_asset_count,
                resolved_asset_count: inventory
                    .acquirable_assets
                    .len()
                    .saturating_sub(unresolved_asset_count),
                unresolved_asset_count,
                unresolved_unique_content_count: needed.len(),
                verified_candidate_file_count: candidate_state.verified_by_path.len(),
                searched_roots,
                unresolved_assets,
                rejected_candidates: candidate_state.rejected.into_iter().collect(),
            },
        ));
    }

    let mut entries = Vec::with_capacity(inventory.acquirable_assets.len());
    for asset in &inventory.acquirable_assets {
        let source_path = resolved.get(&asset.content).ok_or_else(|| {
            CodegenError::new(format!(
                "internal source discovery plan lost resolved identity {}#{}",
                asset.key.form_id, asset.key.asset_id
            ))
        })?;
        entries.push(EvidenceVaultSourceMapEntry {
            form_id: asset.key.form_id.clone(),
            asset_id: asset.key.asset_id.clone(),
            source_path: path_to_utf8(source_path)?,
        });
    }
    entries.sort_by(|left, right| {
        (&left.form_id, &left.asset_id).cmp(&(&right.form_id, &right.asset_id))
    });
    let source_map = EvidenceVaultSourceMap {
        format: EVIDENCE_VAULT_SOURCE_MAP_FORMAT.to_owned(),
        canonicalization: CANONICALIZATION_ID.to_owned(),
        entries,
    };
    let source_map_bytes = canonical_serialize(&source_map, "evidence vault source map")?;
    let source_map_sha256 = sha256_hex(&source_map_bytes);

    if !options.dry_run {
        write_fresh_source_map_file(&output_path, &source_map_bytes)?;
    }

    Ok(DiscoverEvidenceVaultSourcesReport {
        manifest_count: inventory.manifest_count,
        declared_asset_count: inventory.declared_asset_count,
        acquirable_asset_count: inventory.acquirable_assets.len(),
        metadata_only_asset_count: inventory.metadata_only_asset_count,
        zero_size_asset_count: inventory.zero_size_asset_count,
        unique_content_count: groups.len(),
        deduplicated_asset_count: inventory
            .acquirable_assets
            .len()
            .saturating_sub(groups.len()),
        verified_candidate_file_count: candidate_state.verified_by_path.len(),
        rejected_candidates: candidate_state.rejected.into_iter().collect(),
        source_map,
        source_map_sha256,
        output_path,
        written: !options.dry_run,
    })
}

fn load_manifest_inventory(repo_root: &Path) -> CodegenResult<ManifestInventory> {
    let forms_root_path = repo_root.join("rules/forms");
    reject_symlink_ancestors(&forms_root_path, "tracked v1 forms root")?;
    let forms_metadata = fs::symlink_metadata(&forms_root_path).map_err(|source| {
        CodegenError::io("inspect tracked v1 forms root", &forms_root_path, source)
    })?;
    if is_symlink_or_reparse_point(&forms_metadata) || !forms_metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "tracked v1 forms root `{}` must be a real directory",
            forms_root_path.display()
        )));
    }
    let forms_root = fs::canonicalize(&forms_root_path).map_err(|source| {
        CodegenError::io(
            "canonicalize tracked v1 forms root",
            &forms_root_path,
            source,
        )
    })?;
    if !is_same_or_below(repo_root, &forms_root) {
        return Err(CodegenError::new(format!(
            "tracked v1 forms root `{}` escapes repository `{}`",
            forms_root.display(),
            repo_root.display()
        )));
    }

    let mut form_entries = fs::read_dir(&forms_root)
        .map_err(|source| CodegenError::io("read tracked v1 forms root", &forms_root, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CodegenError::io("read tracked v1 form entry", &forms_root, source))?;
    form_entries.sort_by_key(|entry| entry.file_name());
    if form_entries.len() != EXPECTED_V1_FORM_MANIFEST_COUNT {
        return Err(CodegenError::new(format!(
            "vault source discovery requires exactly {EXPECTED_V1_FORM_MANIFEST_COUNT} tracked v1 form directories; found {}",
            form_entries.len()
        )));
    }

    let mut form_ids = BTreeSet::new();
    let mut all_asset_keys = BTreeSet::new();
    let mut acquirable_assets = Vec::new();
    let mut declared_asset_count = 0_usize;
    let mut metadata_only_asset_count = 0_usize;
    let mut zero_size_asset_count = 0_usize;
    let mut forbidden_locator_leaves = BTreeSet::new();
    let mut forbidden_absolute_locators = BTreeSet::new();
    let mut size_by_hash = BTreeMap::<String, u64>::new();

    for entry in form_entries {
        let form_root = entry.path();
        let form_metadata = fs::symlink_metadata(&form_root).map_err(|source| {
            CodegenError::io("inspect tracked v1 form root", &form_root, source)
        })?;
        if is_symlink_or_reparse_point(&form_metadata) || !form_metadata.is_dir() {
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
        reject_symlink_ancestors(&manifest_path, "tracked v1 manifest")?;
        let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|source| {
            CodegenError::io("inspect tracked v1 manifest", &manifest_path, source)
        })?;
        if is_symlink_or_reparse_point(&manifest_metadata) || !manifest_metadata.is_file() {
            return Err(CodegenError::new(format!(
                "tracked v1 manifest `{}` must be a real regular file",
                manifest_path.display()
            )));
        }
        let manifest_bytes = fs::read(&manifest_path).map_err(|source| {
            CodegenError::io("read tracked v1 manifest", &manifest_path, source)
        })?;
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
        let mut form_asset_ids = BTreeSet::new();
        for (index, value) in official_assets.iter().enumerate() {
            let object = value.as_object().ok_or_else(|| {
                CodegenError::new(format!(
                    "{form_id} official_assets[{index}] must be an object"
                ))
            })?;
            let asset_id = required_string(object, "asset_id", "official asset")?.to_owned();
            validate_portable_identifier(&asset_id, "official asset_id")?;
            if !form_asset_ids.insert(asset_id.clone()) {
                return Err(CodegenError::new(format!(
                    "{form_id} contains duplicate official asset_id `{asset_id}`"
                )));
            }
            let key = AssetKey {
                form_id: form_id.clone(),
                asset_id,
            };
            if !all_asset_keys.insert(key.clone()) {
                return Err(CodegenError::new(format!(
                    "tracked manifests contain duplicate asset identity {}#{}",
                    key.form_id, key.asset_id
                )));
            }
            let kind = required_string(object, "kind", "official asset")?.to_owned();
            let sha256 = required_string(object, "sha256", "official asset")?.to_owned();
            validate_sha256(
                &sha256,
                &format!("{} official asset `{}` sha256", key.form_id, key.asset_id),
            )?;
            let size_bytes = object.get("size").and_then(Value::as_u64).ok_or_else(|| {
                CodegenError::new(format!(
                    "{} official asset `{}` size must be a non-negative integer",
                    key.form_id, key.asset_id
                ))
            })?;
            let manifest_locator = required_string(object, "path", "official asset")?.to_owned();
            if manifest_locator.is_empty()
                || manifest_locator.len() > 32_768
                || manifest_locator.chars().any(|character| character == '\0')
            {
                return Err(CodegenError::new(format!(
                    "{} official asset `{}` path must be non-empty, bounded, and NUL-free",
                    key.form_id, key.asset_id
                )));
            }

            declared_asset_count = declared_asset_count
                .checked_add(1)
                .ok_or_else(|| CodegenError::new("tracked official asset count exceeds usize"))?;
            match vault_asset_disposition(&kind, size_bytes)? {
                VaultAssetDisposition::Acquirable => {
                    if let Some(previous_size) = size_by_hash.insert(sha256.clone(), size_bytes)
                        && previous_size != size_bytes
                    {
                        return Err(CodegenError::new(format!(
                            "manifest sha256 `{sha256}` is paired with conflicting sizes {previous_size} and {size_bytes}"
                        )));
                    }
                    acquirable_assets.push(ExpectedAsset {
                        key,
                        kind,
                        content: ContentKey { sha256, size_bytes },
                        manifest_locator,
                    });
                }
                VaultAssetDisposition::MetadataOnlyTaxpayerPayload => {
                    metadata_only_asset_count =
                        metadata_only_asset_count.checked_add(1).ok_or_else(|| {
                            CodegenError::new("metadata-only official asset count exceeds usize")
                        })?;
                    remember_forbidden_locator(
                        &manifest_locator,
                        &mut forbidden_locator_leaves,
                        &mut forbidden_absolute_locators,
                    );
                }
                VaultAssetDisposition::ZeroSizeProvenance => {
                    zero_size_asset_count =
                        zero_size_asset_count.checked_add(1).ok_or_else(|| {
                            CodegenError::new("zero-size official asset count exceeds usize")
                        })?;
                    remember_forbidden_locator(
                        &manifest_locator,
                        &mut forbidden_locator_leaves,
                        &mut forbidden_absolute_locators,
                    );
                }
            }
        }
    }

    if acquirable_assets.is_empty() {
        return Err(CodegenError::new(
            "vault source discovery found no acquirable nonzero official assets",
        ));
    }
    acquirable_assets.sort_by(|left, right| left.key.cmp(&right.key));
    let allowed_locator_leaves = acquirable_assets
        .iter()
        .filter_map(|asset| manifest_locator_leaf(&asset.manifest_locator))
        .map(str::to_ascii_lowercase)
        .collect::<BTreeSet<_>>();
    if allowed_locator_leaves.is_empty() {
        return Err(CodegenError::new(
            "acquirable manifest assets expose no portable locator leaves",
        ));
    }
    Ok(ManifestInventory {
        manifest_count: form_ids.len(),
        declared_asset_count,
        metadata_only_asset_count,
        zero_size_asset_count,
        acquirable_assets,
        allowed_locator_leaves,
        forbidden_locator_leaves,
        forbidden_absolute_locators,
    })
}

fn remember_forbidden_locator(
    locator: &str,
    leaves: &mut BTreeSet<String>,
    absolute_locators: &mut BTreeSet<String>,
) {
    if let Some(leaf) = manifest_locator_leaf(locator) {
        leaves.insert(leaf.to_ascii_lowercase());
    }
    if let Some(path) = native_absolute_manifest_path(locator) {
        absolute_locators.insert(path_comparison_key(&path));
    }
}

fn group_expected_assets(assets: &[ExpectedAsset]) -> BTreeMap<ContentKey, Vec<&ExpectedAsset>> {
    let mut groups = BTreeMap::<ContentKey, Vec<&ExpectedAsset>>::new();
    for asset in assets {
        groups.entry(asset.content.clone()).or_default().push(asset);
    }
    groups
}

fn discover_exact_manifest_candidates(
    repo_root: &Path,
    groups: &BTreeMap<ContentKey, Vec<&ExpectedAsset>>,
    expected_sizes: &BTreeSet<u64>,
    forbidden_leaves: &BTreeSet<String>,
    forbidden_absolute_locators: &BTreeSet<String>,
    resolved: &mut BTreeMap<ContentKey, PathBuf>,
    state: &mut CandidateState,
) -> CodegenResult<()> {
    for (expected, assets) in groups {
        let locators = assets
            .iter()
            .map(|asset| asset.manifest_locator.as_str())
            .collect::<BTreeSet<_>>();
        for locator in locators {
            let Some(path) = native_absolute_manifest_path(locator) else {
                continue;
            };
            let observed = inspect_candidate(
                repo_root,
                &path,
                expected_sizes,
                forbidden_leaves,
                forbidden_absolute_locators,
                state,
            )?;
            let Some(observed) = observed else {
                continue;
            };
            if &observed.content == expected {
                resolved.insert(expected.clone(), observed.path);
                break;
            }
            state.reject(
                &path,
                format!(
                    "exact manifest candidate mismatch: expected {} bytes/{} observed {} bytes/{}",
                    expected.size_bytes,
                    expected.sha256,
                    observed.content.size_bytes,
                    observed.content.sha256
                ),
            );
        }
    }
    Ok(())
}

fn discover_in_search_roots(
    repo_root: &Path,
    roots: &[PathBuf],
    allowed_locator_leaves: &BTreeSet<String>,
    forbidden_leaves: &BTreeSet<String>,
    forbidden_absolute_locators: &BTreeSet<String>,
    needed: &mut BTreeSet<ContentKey>,
    resolved: &mut BTreeMap<ContentKey, PathBuf>,
    state: &mut CandidateState,
) -> CodegenResult<()> {
    for root in roots {
        if needed.is_empty() {
            break;
        }
        visit_search_directory(
            repo_root,
            root,
            allowed_locator_leaves,
            forbidden_leaves,
            forbidden_absolute_locators,
            needed,
            resolved,
            state,
        )?;
    }
    Ok(())
}

fn visit_search_directory(
    repo_root: &Path,
    directory: &Path,
    allowed_locator_leaves: &BTreeSet<String>,
    forbidden_leaves: &BTreeSet<String>,
    forbidden_absolute_locators: &BTreeSet<String>,
    needed: &mut BTreeSet<ContentKey>,
    resolved: &mut BTreeMap<ContentKey, PathBuf>,
    state: &mut CandidateState,
) -> CodegenResult<()> {
    if needed.is_empty() {
        return Ok(());
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => match entries.collect::<std::result::Result<Vec<_>, _>>() {
            Ok(entries) => entries,
            Err(source) => {
                state.reject(
                    directory,
                    format!("could not enumerate search directory: {source}"),
                );
                return Ok(());
            }
        },
        Err(source) => {
            state.reject(
                directory,
                format!("could not read search directory: {source}"),
            );
            return Ok(());
        }
    };
    let mut entries = entries;
    entries.sort_by_key(|entry| path_sort_key(&entry.path()));

    for entry in entries {
        if needed.is_empty() {
            break;
        }
        let path = entry.path();
        if let Some(reason) =
            sensitive_path_reason(&path, forbidden_leaves, forbidden_absolute_locators)
        {
            state.reject(&path, reason);
            continue;
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(source) => {
                state.reject(&path, format!("could not inspect search entry: {source}"));
                continue;
            }
        };
        if is_symlink_or_reparse_point(&metadata) {
            state.reject(&path, "symlink/reparse search entry was not followed");
            continue;
        }
        if metadata.is_dir() {
            visit_search_directory(
                repo_root,
                &path,
                allowed_locator_leaves,
                forbidden_leaves,
                forbidden_absolute_locators,
                needed,
                resolved,
                state,
            )?;
            continue;
        }
        if !metadata.is_file() || metadata.len() == 0 {
            continue;
        }
        let candidate_leaf = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_ascii_lowercase);
        if !candidate_leaf
            .as_ref()
            .is_some_and(|leaf| allowed_locator_leaves.contains(leaf))
        {
            // Never open arbitrary same-sized files. Search-root discovery is
            // limited to exact acquirable manifest leaf names; renamed files
            // must be supplied through an explicit source map.
            continue;
        }
        if !needed
            .iter()
            .any(|content| content.size_bytes == metadata.len())
        {
            continue;
        }
        let expected_sizes = needed
            .iter()
            .map(|content| content.size_bytes)
            .collect::<BTreeSet<_>>();
        let Some(observed) = inspect_candidate(
            repo_root,
            &path,
            &expected_sizes,
            forbidden_leaves,
            forbidden_absolute_locators,
            state,
        )?
        else {
            continue;
        };
        if needed.remove(&observed.content) {
            resolved.insert(observed.content.clone(), observed.path);
        }
    }
    Ok(())
}

fn inspect_candidate(
    repo_root: &Path,
    path: &Path,
    expected_sizes: &BTreeSet<u64>,
    forbidden_leaves: &BTreeSet<String>,
    forbidden_absolute_locators: &BTreeSet<String>,
    state: &mut CandidateState,
) -> CodegenResult<Option<ObservedFile>> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        state.reject(
            path,
            "candidate path is not absolute and lexically normalized",
        );
        return Ok(None);
    }
    if let Some(reason) = sensitive_path_reason(path, forbidden_leaves, forbidden_absolute_locators)
    {
        state.reject(path, reason);
        return Ok(None);
    }
    if let Err(error) = reject_symlink_ancestors(path, "vault source candidate") {
        state.reject(path, error.to_string());
        return Ok(None);
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            state.reject(path, format!("could not inspect candidate: {source}"));
            return Ok(None);
        }
    };
    if is_symlink_or_reparse_point(&metadata) {
        state.reject(path, "candidate is a symlink/reparse point");
        return Ok(None);
    }
    if !metadata.is_file() {
        state.reject(path, "candidate is not a regular file");
        return Ok(None);
    }
    if metadata.len() == 0 || !expected_sizes.contains(&metadata.len()) {
        return Ok(None);
    }
    let canonical = match fs::canonicalize(path) {
        Ok(path) => path,
        Err(source) => {
            state.reject(path, format!("could not canonicalize candidate: {source}"));
            return Ok(None);
        }
    };
    if is_same_or_below(repo_root, &canonical) {
        state.reject(
            &canonical,
            format!("candidate is inside repository `{}`", repo_root.display()),
        );
        return Ok(None);
    }
    if let Some(reason) =
        sensitive_path_reason(&canonical, forbidden_leaves, forbidden_absolute_locators)
    {
        state.reject(&canonical, reason);
        return Ok(None);
    }
    if let Some(observed) = state.verified_by_path.get(&canonical) {
        return Ok(Some(observed.clone()));
    }
    let content = match hash_regular_file(
        repo_root,
        &canonical,
        forbidden_leaves,
        forbidden_absolute_locators,
    ) {
        Ok(content) => content,
        Err(error) => {
            state.reject(&canonical, error.to_string());
            return Ok(None);
        }
    };
    let observed = ObservedFile {
        path: canonical.clone(),
        content,
    };
    state.verified_by_path.insert(canonical, observed.clone());
    Ok(Some(observed))
}

fn hash_regular_file(
    repo_root: &Path,
    path: &Path,
    forbidden_leaves: &BTreeSet<String>,
    forbidden_absolute_locators: &BTreeSet<String>,
) -> CodegenResult<ContentKey> {
    let mut verified = open_verified_regular_file(path, "vault source candidate", |resolved| {
        if is_same_or_below(repo_root, resolved) {
            return Err(CodegenError::new(format!(
                "vault source candidate `{}` resolves inside repository `{}`",
                resolved.display(),
                repo_root.display()
            )));
        }
        if let Some(reason) =
            sensitive_path_reason(resolved, forbidden_leaves, forbidden_absolute_locators)
        {
            return Err(CodegenError::new(format!(
                "vault source candidate `{}` is forbidden: {reason}",
                resolved.display()
            )));
        }
        Ok(())
    })?;
    let metadata = verified.file().metadata().map_err(|source| {
        CodegenError::io(
            "inspect verified vault source candidate",
            verified.canonical_path(),
            source,
        )
    })?;
    if metadata.len() == 0 {
        return Err(CodegenError::new(format!(
            "vault source candidate `{}` must not be zero-size",
            path.display()
        )));
    }
    let mut digest = Sha256::new();
    let mut size_bytes = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = verified
            .file_mut()
            .read(&mut buffer)
            .map_err(|source| CodegenError::io("read vault source candidate", path, source))?;
        if count == 0 {
            break;
        }
        size_bytes = size_bytes
            .checked_add(count as u64)
            .ok_or_else(|| CodegenError::new("vault source candidate exceeds u64 byte count"))?;
        digest.update(&buffer[..count]);
    }
    Ok(ContentKey {
        sha256: encode_digest(digest.finalize()),
        size_bytes,
    })
}

fn validate_search_roots(repo_root: &Path, roots: &[PathBuf]) -> CodegenResult<Vec<PathBuf>> {
    let mut canonical_roots = Vec::with_capacity(roots.len());
    for root in roots {
        require_absolute_normalized(root, "vault source search root")?;
        if is_filesystem_or_share_root(root) {
            return Err(CodegenError::new(format!(
                "vault source search root `{}` is a filesystem, drive, or UNC share root; supply a dedicated official-assets subdirectory",
                root.display()
            )));
        }
        if is_disallowed_parallels_home_search_root(root) {
            return Err(CodegenError::new(
                "vault source search root `C:\\Mac\\Home` is a redirected host-home alias; supply an explicit real directory or UNC search root",
            ));
        }
        if is_broad_home_search_root(root) {
            return Err(CodegenError::new(format!(
                "vault source search root `{}` is a broad user-home directory; supply a dedicated official-assets subdirectory",
                root.display()
            )));
        }
        reject_symlink_ancestors(root, "vault source search root")?;
        if sensitive_path_reason(root, &BTreeSet::new(), &BTreeSet::new()).is_some() {
            return Err(CodegenError::new(format!(
                "vault source search root `{}` is beneath a known taxpayer/save/live-database root",
                root.display()
            )));
        }
        let metadata = fs::symlink_metadata(root)
            .map_err(|source| CodegenError::io("inspect vault source search root", root, source))?;
        if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
            return Err(CodegenError::new(format!(
                "vault source search root `{}` must be a real directory",
                root.display()
            )));
        }
        let canonical = fs::canonicalize(root).map_err(|source| {
            CodegenError::io("canonicalize vault source search root", root, source)
        })?;
        if is_same_or_below(repo_root, &canonical) {
            return Err(CodegenError::new(format!(
                "vault source search root `{}` must remain outside repository `{}`",
                root.display(),
                repo_root.display()
            )));
        }
        if sensitive_path_reason(&canonical, &BTreeSet::new(), &BTreeSet::new()).is_some() {
            return Err(CodegenError::new(format!(
                "vault source search root `{}` resolves beneath a known taxpayer/save/live-database root",
                root.display()
            )));
        }
        canonical_roots.push(canonical);
    }
    canonical_roots.sort_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| path_sort_key(left).cmp(&path_sort_key(right)))
    });
    canonical_roots.dedup_by(|left, right| path_comparison_key(left) == path_comparison_key(right));
    let mut non_overlapping = Vec::<PathBuf>::new();
    for root in canonical_roots {
        if non_overlapping
            .iter()
            .any(|parent| is_same_or_below(parent, &root))
        {
            continue;
        }
        non_overlapping.push(root);
    }
    non_overlapping.sort_by_key(|path| path_sort_key(path));
    Ok(non_overlapping)
}

fn validate_fresh_external_output(repo_root: &Path, target: &Path) -> CodegenResult<PathBuf> {
    require_absolute_normalized(target, "vault source-map output")?;
    if is_same_or_below(repo_root, target) {
        return Err(CodegenError::new(format!(
            "vault source-map output `{}` must remain outside repository `{}`",
            target.display(),
            repo_root.display()
        )));
    }
    reject_symlink_ancestors(target, "vault source-map output")?;
    if sensitive_path_reason(target, &BTreeSet::new(), &BTreeSet::new()).is_some() {
        return Err(CodegenError::new(format!(
            "vault source-map output `{}` is beneath a known taxpayer/save/live-database root",
            target.display()
        )));
    }
    match fs::symlink_metadata(target) {
        Ok(_) => {
            return Err(CodegenError::new(format!(
                "vault source-map output `{}` already exists; refusing to overwrite",
                target.display()
            )));
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(CodegenError::io(
                "inspect vault source-map output",
                target,
                source,
            ));
        }
    }
    let parent = target.parent().ok_or_else(|| {
        CodegenError::new(format!(
            "vault source-map output `{}` has no parent",
            target.display()
        ))
    })?;
    reject_symlink_ancestors(parent, "vault source-map output parent")?;
    let metadata = fs::symlink_metadata(parent).map_err(|source| {
        CodegenError::io("inspect vault source-map output parent", parent, source)
    })?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "vault source-map output parent `{}` must be a real directory",
            parent.display()
        )));
    }
    let canonical_parent = fs::canonicalize(parent).map_err(|source| {
        CodegenError::io(
            "canonicalize vault source-map output parent",
            parent,
            source,
        )
    })?;
    if is_same_or_below(repo_root, &canonical_parent) {
        return Err(CodegenError::new(format!(
            "vault source-map output parent `{}` resolves beneath repository `{}`",
            parent.display(),
            repo_root.display()
        )));
    }
    let file_name = target
        .file_name()
        .ok_or_else(|| CodegenError::new("vault source-map output must have a final file name"))?;
    let file_name_text = file_name.to_str().ok_or_else(|| {
        CodegenError::new("vault source-map output file name must be valid UTF-8")
    })?;
    if file_name_text.is_empty() || file_name_text.chars().any(char::is_control) {
        return Err(CodegenError::new(
            "vault source-map output file name must be non-empty and control-free",
        ));
    }
    let canonical_target = canonical_parent.join(file_name);
    if sensitive_path_reason(&canonical_target, &BTreeSet::new(), &BTreeSet::new()).is_some() {
        return Err(CodegenError::new(format!(
            "vault source-map output `{}` resolves beneath a known taxpayer/save/live-database root",
            target.display()
        )));
    }
    Ok(canonical_target)
}

fn write_fresh_source_map_file(target: &Path, bytes: &[u8]) -> CodegenResult<()> {
    write_fresh_source_map_file_with_hook(target, bytes, |_| Ok(()))
}

fn write_fresh_source_map_file_with_hook<F>(
    target: &Path,
    bytes: &[u8],
    after_create: F,
) -> CodegenResult<()>
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    match fs::symlink_metadata(target) {
        Ok(_) => {
            return Err(CodegenError::new(format!(
                "vault source-map output `{}` already exists; refusing to overwrite",
                target.display()
            )));
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(CodegenError::io(
                "inspect vault source-map output before install",
                target,
                source,
            ));
        }
    }
    reject_symlink_ancestors(target, "vault source-map output")?;
    let parent = target
        .parent()
        .expect("validated source-map output has a parent");
    let parent_handle = Handle::from_path(parent).map_err(|source| {
        CodegenError::io(
            "identify vault source-map output parent before create",
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
                "vault source-map output `{}` appeared before create; refusing to overwrite",
                target.display()
            )));
        }
        Err(source) => {
            return Err(CodegenError::io(
                "create fresh vault source-map output",
                target,
                source,
            ));
        }
    };
    let opened_handle = (|| {
        let cloned = output_file.try_clone().map_err(|source| {
            CodegenError::io("clone fresh vault source-map output handle", target, source)
        })?;
        Handle::from_file(cloned).map_err(|source| {
            CodegenError::io(
                "identify fresh vault source-map output handle",
                target,
                source,
            )
        })
    })()
    .map_err(|source| incomplete_fresh_output_error(target, "vault source-map", source))?;

    // The final name is created directly and is incomplete until all checks
    // finish. No error path removes it by name because that path may have been
    // substituted by another same-user process.
    let operation = (|| {
        after_create(target).map_err(|source| {
            CodegenError::io(
                "run vault source-map post-create verification hook",
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
            "vault source-map output",
        )?;
        output_file.write_all(bytes).map_err(|source| {
            CodegenError::io("write fresh vault source-map output", target, source)
        })?;
        output_file.sync_all().map_err(|source| {
            CodegenError::io("sync fresh vault source-map output", target, source)
        })?;
        verify_canonical_source_map_file(&mut output_file, target, bytes)?;
        verify_fresh_output_identity(
            target,
            parent,
            &parent_handle,
            &opened_handle,
            &output_file,
            "vault source-map output",
        )?;
        sync_directory(parent)?;
        verify_fresh_output_identity(
            target,
            parent,
            &parent_handle,
            &opened_handle,
            &output_file,
            "vault source-map output",
        )?;
        Ok(())
    })();
    operation.map_err(|source| incomplete_fresh_output_error(target, "vault source-map", source))
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

fn verify_canonical_source_map_file(
    file: &mut File,
    path: &Path,
    expected: &[u8],
) -> CodegenResult<()> {
    file.seek(SeekFrom::Start(0))
        .map_err(|source| CodegenError::io("rewind fresh vault source map", path, source))?;
    let mut actual = Vec::new();
    file.read_to_end(&mut actual)
        .map_err(|source| CodegenError::io("read fresh vault source map handle", path, source))?;
    if actual != expected {
        return Err(CodegenError::new(
            "fresh vault source-map bytes drifted after write",
        ));
    }
    let parsed = parse_strict(&actual, path)?;
    if canonical_bytes(&parsed) != actual {
        return Err(CodegenError::new(
            "fresh vault source map is not canonical JSON",
        ));
    }
    let map: EvidenceVaultSourceMap =
        serde_json::from_value(parsed.into_serde()).map_err(|source| {
            CodegenError::with_source(
                "closed-structure load of fresh vault source map failed",
                source,
            )
        })?;
    if map.format != EVIDENCE_VAULT_SOURCE_MAP_FORMAT
        || map.canonicalization != CANONICALIZATION_ID
        || map.entries.is_empty()
    {
        return Err(CodegenError::new(
            "fresh vault source map has an invalid header or no entries",
        ));
    }
    Ok(())
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
) -> CodegenResult<()> {
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
    reject_fresh_output_hard_link_alias(opened_file, &opened_metadata, target, label)
}

#[cfg(unix)]
fn reject_fresh_output_hard_link_alias(
    _file: &File,
    metadata: &Metadata,
    path: &Path,
    label: &str,
) -> CodegenResult<()> {
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
fn reject_fresh_output_hard_link_alias(
    file: &File,
    _metadata: &Metadata,
    path: &Path,
    label: &str,
) -> CodegenResult<()> {
    let link_count = stable_windows_link_count(file, path, label)?;
    if link_count != 1 {
        return Err(CodegenError::new(format!(
            "{label} `{path}` has {link_count} hard links; aliased fresh outputs are forbidden",
            path = path.display()
        )));
    }
    Ok(())
}

fn native_absolute_manifest_path(locator: &str) -> Option<PathBuf> {
    let lower = locator.to_ascii_lowercase();
    if lower.starts_with("http:")
        || lower.starts_with("https:")
        || lower.starts_with("file:")
        || locator.chars().any(char::is_control)
    {
        return None;
    }
    let path = PathBuf::from(locator);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return None;
    }
    Some(path)
}

fn manifest_locator_leaf(locator: &str) -> Option<&str> {
    locator
        .rsplit(|character| character == '/' || character == '\\')
        .find(|component| !component.is_empty())
}

fn sensitive_path_reason(
    path: &Path,
    forbidden_leaves: &BTreeSet<String>,
    forbidden_absolute_locators: &BTreeSet<String>,
) -> Option<&'static str> {
    if forbidden_absolute_locators.contains(&path_comparison_key(path)) {
        return Some("candidate is the declared locator of a metadata-only/zero-size asset");
    }
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_ascii_lowercase()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let leaf = components.last();
    if leaf.is_some_and(|leaf| forbidden_leaves.contains(leaf)) {
        return Some("candidate leaf is declared by a metadata-only/zero-size asset");
    }
    let normalized_leaf = leaf
        .map(|leaf| leaf.replace(' ', "-").replace('_', "-"))
        .unwrap_or_default();
    if normalized_leaf.contains("final-copy")
        || normalized_leaf.contains("taxpayer")
        || normalized_leaf.contains("savefile")
        || normalized_leaf.contains("saved-return")
        || normalized_leaf.contains("return-payload")
        || normalized_leaf.contains("submission-payload")
        || normalized_leaf.contains("editable-save")
        || normalized_leaf.contains("finalized-save")
        || normalized_leaf.contains("plaintext-save")
        || normalized_leaf.contains("representative-save")
        || normalized_leaf.contains("encrypted-copy")
        || normalized_leaf.contains("-save-")
        || normalized_leaf.ends_with("-save.xml")
    {
        return Some("candidate name is taxpayer/save/final-copy shaped");
    }
    let has_pair = |left: &str, right: &str| {
        components
            .windows(2)
            .any(|pair| pair[0] == left && pair[1] == right)
    };
    let has_sensitive_component = components.iter().any(|component| {
        let normalized = component.replace(' ', "-").replace('_', "-");
        normalized.contains("taxpayer")
            || normalized.contains("tax-payer")
            || normalized.contains("final-copy")
            || normalized.contains("finalcopy")
            || normalized.starts_with("save-")
            || normalized.ends_with("-save")
            || normalized.contains("-save-")
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
                    | "secrets"
            )
    });
    if has_pair("ebirforms", "savefile")
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
        })
    {
        return Some("candidate is beneath a known taxpayer/save/live-database root");
    }
    None
}

fn is_disallowed_parallels_home_search_root(path: &Path) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let key = path_comparison_key(path);
    let key = key.trim_end_matches('/');
    key == "c:/mac/home" || key == "//?/c:/mac/home"
}

fn is_broad_home_search_root(path: &Path) -> bool {
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_ascii_lowercase()),
            _ => None,
        })
        .collect::<Vec<_>>();
    match components.as_slice() {
        [single] if single == "home" || single == "users" => true,
        [parent, _user] if parent == "home" || parent == "users" => true,
        [_, parent, _user] if parent == "home" || parent == "users" => true,
        _ => false,
    }
}

fn is_filesystem_or_share_root(path: &Path) -> bool {
    // Absolute filesystem roots contain only a platform prefix and/or root
    // separator. This covers `/`, Windows drive/volume roots, and UNC share
    // roots while allowing a dedicated directory beneath any of them.
    !path
        .components()
        .any(|component| matches!(component, Component::Normal(_)))
}

fn require_absolute_normalized(path: &Path, label: &str) -> CodegenResult<()> {
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

fn reject_symlink_ancestors(path: &Path, label: &str) -> CodegenResult<()> {
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

fn required_array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    label: &str,
) -> CodegenResult<&'a Vec<Value>> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| CodegenError::new(format!("{label} is missing required array `{key}`")))
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    label: &str,
) -> CodegenResult<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| CodegenError::new(format!("{label} is missing required string `{key}`")))
}

fn validate_portable_identifier(value: &str, label: &str) -> CodegenResult<()> {
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

fn validate_sha256(value: &str, label: &str) -> CodegenResult<()> {
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

fn canonical_serialize(value: &impl Serialize, label: &str) -> CodegenResult<Vec<u8>> {
    let ordinary = serde_json::to_vec(value)
        .map_err(|source| CodegenError::with_source(format!("serialize {label}"), source))?;
    let parsed = parse_strict(&ordinary, Path::new(label))?;
    Ok(canonical_bytes(&parsed))
}

fn path_to_utf8(path: &Path) -> CodegenResult<String> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        CodegenError::new(format!(
            "verified vault source path `{}` is not valid UTF-8",
            path.display()
        ))
    })
}

fn path_comparison_key(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        value.to_ascii_lowercase()
    } else {
        value
    }
}

fn path_sort_key(path: &Path) -> String {
    path_comparison_key(path)
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

fn sync_directory(path: &Path) -> CodegenResult<()> {
    match File::open(path).and_then(|directory| directory.sync_all()) {
        Ok(()) => Ok(()),
        Err(source) if cfg!(windows) => {
            let _ = source;
            Ok(())
        }
        Err(source) => Err(CodegenError::io(
            "sync vault source-map output directory",
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
    use std::thread;

    use serde_json::{Value, json};

    use super::{
        DiscoverEvidenceVaultSourcesOptions, EXPECTED_V1_FORM_MANIFEST_COUNT,
        EvidenceVaultSourceDiscoveryError, canonical_serialize, discover_evidence_vault_sources,
        sha256_hex, write_fresh_source_map_file, write_fresh_source_map_file_with_hook,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
    const OFFICIAL_BYTES: &[u8] = b"synthetic static official package bytes";
    const WRONG_SAME_SIZE_BYTES: &[u8] = b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

    struct TestRoot {
        path: PathBuf,
    }

    impl TestRoot {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "bir-vault-source-discovery-{label}-{}-{}",
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
        input_root: PathBuf,
        exact_source: PathBuf,
    }

    impl Fixture {
        fn new(label: &str, exact_locators: bool) -> Self {
            let root = TestRoot::new(label);
            let repo_root = root.path.join("repo");
            let forms_root = repo_root.join("rules/forms");
            let input_root = root.path.join("external-input");
            fs::create_dir_all(&forms_root).expect("create fixture forms");
            fs::create_dir(&input_root).expect("create fixture input");
            let exact_source = input_root.join("official-package.bin");
            fs::write(&exact_source, OFFICIAL_BYTES).expect("write official bytes");
            let hash = sha256_hex(OFFICIAL_BYTES);

            for ordinal in 1..=EXPECTED_V1_FORM_MANIFEST_COUNT {
                let form_id = format!("form-{ordinal:03}");
                let asset_id = format!("package-{ordinal:03}");
                let form_root = forms_root.join(&form_id);
                fs::create_dir(&form_root).expect("create form root");
                let manifest_locator = if exact_locators {
                    exact_source.to_string_lossy().into_owned()
                } else {
                    format!("Z:\\stale\\official-package-{ordinal:03}.bin")
                };
                let mut assets = vec![json!({
                    "asset_id": asset_id,
                    "kind": "official-package-executable",
                    "path": manifest_locator,
                    "sha256": hash,
                    "size": OFFICIAL_BYTES.len()
                })];
                if ordinal == 1 {
                    assets.push(json!({
                        "asset_id": "dummy-final-copy",
                        "kind": "dummy-profile-encrypted-final-copy",
                        "path": input_root.join("must-not-open-final-copy.xml"),
                        "sha256": "f".repeat(64),
                        "size": OFFICIAL_BYTES.len()
                    }));
                }
                if ordinal == 2 {
                    assets.push(json!({
                        "asset_id": "zero-provenance",
                        "kind": "prior-reviewed-repository-provenance",
                        "path": input_root.join("must-not-open-zero.bin"),
                        "sha256": "0".repeat(64),
                        "size": 0
                    }));
                }
                write_json(
                    &form_root.join("manifest.json"),
                    &json!({
                        "form_id": form_id,
                        "official_assets": assets
                    }),
                );
            }
            Self {
                _root: root,
                repo_root,
                input_root,
                exact_source,
            }
        }

        fn output(&self, name: &str) -> PathBuf {
            self._root.path.join(name)
        }

        fn options(
            &self,
            output_name: &str,
            search_roots: Vec<PathBuf>,
            dry_run: bool,
        ) -> DiscoverEvidenceVaultSourcesOptions {
            let mut options =
                DiscoverEvidenceVaultSourcesOptions::new(&self.repo_root, self.output(output_name));
            options.search_roots = search_roots;
            options.dry_run = dry_run;
            options
        }
    }

    fn write_json(path: &Path, value: &Value) {
        fs::write(
            path,
            canonical_serialize(value, "test manifest").expect("canonical fixture JSON"),
        )
        .expect("write fixture JSON");
    }

    #[test]
    fn exact_43_form_discovery_is_complete_deduplicated_and_no_write() {
        let fixture = Fixture::new("exact-dry-run", true);
        let options = fixture.options("source-map.json", Vec::new(), true);
        let report =
            discover_evidence_vault_sources(&options).expect("discover exact fixture paths");
        assert_eq!(report.manifest_count, EXPECTED_V1_FORM_MANIFEST_COUNT);
        assert_eq!(report.declared_asset_count, 45);
        assert_eq!(
            report.acquirable_asset_count,
            EXPECTED_V1_FORM_MANIFEST_COUNT
        );
        assert_eq!(report.metadata_only_asset_count, 1);
        assert_eq!(report.zero_size_asset_count, 1);
        assert_eq!(report.unique_content_count, 1);
        assert_eq!(
            report.deduplicated_asset_count,
            EXPECTED_V1_FORM_MANIFEST_COUNT - 1
        );
        assert_eq!(report.verified_candidate_file_count, 1);
        assert_eq!(
            report.source_map.entries.len(),
            EXPECTED_V1_FORM_MANIFEST_COUNT
        );
        assert!(!report.written);
        assert!(!options.output_path.exists());
        let bytes = report
            .canonical_source_map_bytes()
            .expect("canonical source map");
        assert_eq!(sha256_hex(&bytes), report.source_map_sha256);
    }

    #[test]
    fn explicit_search_root_hashes_candidates_before_mapping() {
        let fixture = Fixture::new("search-root", false);
        let wrong = fixture.input_root.join("official-package-001.bin");
        assert_eq!(WRONG_SAME_SIZE_BYTES.len(), OFFICIAL_BYTES.len());
        fs::write(&wrong, WRONG_SAME_SIZE_BYTES).expect("write same-size decoy");
        let found = fixture.input_root.join("official-package-002.bin");
        fs::rename(&fixture.exact_source, &found).expect("move official fixture source");
        let report = discover_evidence_vault_sources(&fixture.options(
            "source-map.json",
            vec![fixture.input_root.clone()],
            true,
        ))
        .expect("discover under explicit search root");
        let canonical_found = fs::canonicalize(&found).expect("canonical found source");
        assert_eq!(report.verified_candidate_file_count, 2);
        assert!(
            report
                .source_map
                .entries
                .iter()
                .all(|entry| Path::new(&entry.source_path) == canonical_found)
        );
    }

    #[test]
    fn discovery_rejects_hard_link_alias_and_uses_an_independent_copy() {
        let fixture = Fixture::new("hard-link-candidate", false);
        let alias = fixture.input_root.join("official-package-001.bin");
        fs::hard_link(&fixture.exact_source, &alias).expect("create candidate hard-link alias");
        let safe = fixture.input_root.join("official-package-002.bin");
        fs::write(&safe, OFFICIAL_BYTES).expect("write independent safe candidate");

        let report = discover_evidence_vault_sources(&fixture.options(
            "source-map.json",
            vec![fixture.input_root.clone()],
            true,
        ))
        .expect("independent candidate remains discoverable");
        let canonical_alias = fs::canonicalize(&alias).expect("canonical alias");
        let canonical_safe = fs::canonicalize(&safe).expect("canonical safe candidate");
        assert!(
            report
                .source_map
                .entries
                .iter()
                .all(|entry| Path::new(&entry.source_path) == canonical_safe)
        );
        assert!(report.rejected_candidates.iter().any(|candidate| {
            candidate.path.as_str() == canonical_alias.to_string_lossy().as_ref()
                && candidate.reason.contains("hard links")
                && candidate.reason.contains("aliased external inputs")
        }));
    }

    #[test]
    fn unresolved_search_returns_every_asset_and_never_emits_a_partial_map() {
        let fixture = Fixture::new("unresolved", false);
        fs::remove_file(&fixture.exact_source).expect("remove only matching bytes");
        let output = fixture.output("source-map.json");
        let error = discover_evidence_vault_sources(&fixture.options(
            "source-map.json",
            vec![fixture.input_root.clone()],
            false,
        ))
        .expect_err("incomplete discovery must fail");
        let EvidenceVaultSourceDiscoveryError::Unresolved(report) = error else {
            panic!("expected structured unresolved report");
        };
        assert_eq!(
            report.unresolved_asset_count,
            EXPECTED_V1_FORM_MANIFEST_COUNT
        );
        assert_eq!(report.unresolved_unique_content_count, 1);
        assert_eq!(report.unresolved_assets[0].form_id, "form-001");
        assert_eq!(
            report.unresolved_assets.last().expect("last asset").form_id,
            "form-043"
        );
        assert!(!output.exists(), "partial source map must never be written");
    }

    #[test]
    fn complete_output_is_canonical_fresh_and_refuses_overwrite() {
        let fixture = Fixture::new("fresh-output", true);
        let options = fixture.options("source-map.json", Vec::new(), false);
        let report = discover_evidence_vault_sources(&options).expect("write complete source map");
        assert!(report.written);
        assert_eq!(
            fs::read(&options.output_path).expect("read source-map output"),
            report
                .canonical_source_map_bytes()
                .expect("canonical report bytes")
        );

        let original = fs::read(&options.output_path).expect("read original output");
        let error = discover_evidence_vault_sources(&options)
            .expect_err("existing output must fail closed");
        assert!(
            error.to_string().contains("refusing to overwrite"),
            "{error}"
        );
        assert_eq!(
            fs::read(&options.output_path).expect("re-read original output"),
            original
        );
    }

    #[test]
    fn racing_fresh_source_map_writers_never_overwrite_each_other() {
        let fixture = Fixture::new("writer-race", true);
        let dry_run =
            discover_evidence_vault_sources(&fixture.options("unused.json", Vec::new(), true))
                .expect("build canonical source map");
        let bytes = dry_run
            .canonical_source_map_bytes()
            .expect("canonical source-map bytes");
        let target = fixture.output("source-map.json");
        let left_target = target.clone();
        let left_bytes = bytes.clone();
        let left = thread::spawn(move || write_fresh_source_map_file(&left_target, &left_bytes));
        let right_target = target.clone();
        let right_bytes = bytes.clone();
        let right = thread::spawn(move || write_fresh_source_map_file(&right_target, &right_bytes));
        let outcomes = [
            left.join().expect("left writer did not panic"),
            right.join().expect("right writer did not panic"),
        ];
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(fs::read(&target).expect("read winning output"), bytes);
    }

    #[test]
    fn hard_link_alias_after_create_is_rejected_and_neither_name_is_deleted() {
        let root = TestRoot::new("hard-link-output");
        let target = root.path.join("source-map.json");
        let alias = root.path.join("attacker-alias.json");
        let error = write_fresh_source_map_file_with_hook(&target, b"{}", |created| {
            fs::hard_link(created, &alias)
        })
        .expect_err("hard-linked fresh output must fail closed");
        assert!(error.to_string().contains("deliberately left in place"));
        assert!(target.exists(), "owned incomplete output must remain");
        assert!(alias.exists(), "attacker alias must never be removed");
    }

    #[cfg(unix)]
    #[test]
    fn substituted_source_map_target_is_not_deleted() {
        let root = TestRoot::new("substituted-output");
        let target = root.path.join("source-map.json");
        let attacker_bytes = b"attacker-substitute";
        let error = write_fresh_source_map_file_with_hook(&target, b"{}", |created| {
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
    fn metadata_only_and_zero_size_locators_are_never_candidates() {
        let fixture = Fixture::new("metadata-never-open", false);
        let safe_path = fixture.input_root.join("official-package-001.bin");
        fs::rename(&fixture.exact_source, &safe_path).expect("move safe package fixture");
        let metadata_path = fixture.input_root.join("must-not-open-final-copy.xml");
        let zero_path = fixture.input_root.join("must-not-open-zero.bin");
        // These bytes intentionally match the acquirable identity. If either
        // forbidden locator were opened, deterministic search order would map
        // it before reaching the safe package file.
        fs::write(&metadata_path, OFFICIAL_BYTES).expect("write forbidden metadata fixture");
        fs::write(&zero_path, OFFICIAL_BYTES).expect("write forbidden zero-size fixture");

        let report = discover_evidence_vault_sources(&fixture.options(
            "source-map.json",
            vec![fixture.input_root.clone()],
            true,
        ))
        .expect("non-acquirable locators must be inert");
        assert_eq!(report.verified_candidate_file_count, 1);
        let canonical_safe = fs::canonicalize(&safe_path).expect("canonical safe package");
        let canonical_metadata =
            fs::canonicalize(&metadata_path).expect("canonical metadata-only path");
        let canonical_zero = fs::canonicalize(&zero_path).expect("canonical zero-size path");
        assert!(
            report
                .source_map
                .entries
                .iter()
                .all(|entry| Path::new(&entry.source_path) == canonical_safe)
        );
        assert!(
            report
                .rejected_candidates
                .iter()
                .any(|candidate| candidate.path.as_str()
                    == canonical_metadata.to_string_lossy().as_ref()
                    && candidate.reason.contains("metadata-only/zero-size")),
            "{:#?}",
            report.rejected_candidates,
        );
        assert!(
            report
                .rejected_candidates
                .iter()
                .any(|candidate| candidate.path.as_str()
                    == canonical_zero.to_string_lossy().as_ref()
                    && candidate.reason.contains("metadata-only/zero-size"))
        );
    }

    #[test]
    fn sensitive_and_symlink_candidates_are_skipped_for_a_safe_fallback() {
        let fixture = Fixture::new("candidate-rejection", false);
        let safe = fixture.input_root.join("official-package-001.bin");
        fs::rename(&fixture.exact_source, &safe).expect("move safe source");
        let arbitrary_same_size = fixture.input_root.join("credential-looking-random.bin");
        fs::write(&arbitrary_same_size, OFFICIAL_BYTES)
            .expect("write same-size non-manifest candidate");
        let sensitive = fixture.input_root.join("a-final-copy.xml");
        fs::write(&sensitive, OFFICIAL_BYTES).expect("write sensitive candidate");
        let symlink = fixture.input_root.join("b-link.bin");
        match create_file_symlink(&safe, &symlink) {
            Ok(()) => {}
            Err(source)
                if source.kind() == io::ErrorKind::PermissionDenied
                    || source.kind() == io::ErrorKind::Unsupported
                    || source.raw_os_error() == Some(1314) => {}
            Err(source) => panic!("create fixture symlink: {source}"),
        }
        let profile_root = fixture.input_root.join("c-profile");
        fs::create_dir(&profile_root).expect("create forbidden profile root");
        fs::write(
            profile_root.join("official-package-002.bin"),
            OFFICIAL_BYTES,
        )
        .expect("write forbidden profile candidate");

        let report = discover_evidence_vault_sources(&fixture.options(
            "source-map.json",
            vec![fixture.input_root.clone()],
            true,
        ))
        .expect("safe candidate remains discoverable");
        let canonical_input = fs::canonicalize(&fixture.input_root).expect("canonical input root");
        let canonical_safe = fs::canonicalize(&safe).expect("canonical safe source");
        let canonical_sensitive = fs::canonicalize(&sensitive).expect("canonical sensitive source");
        let canonical_profile = fs::canonicalize(&profile_root).expect("canonical profile root");
        assert_eq!(
            report.source_map.entries[0].source_path,
            canonical_safe.to_string_lossy().as_ref()
        );
        assert!(
            !report.rejected_candidates.iter().any(|candidate| {
                candidate.path.as_str() == arbitrary_same_size.to_string_lossy().as_ref()
            }),
            "non-manifest filenames must be skipped without opening"
        );
        assert!(
            report.rejected_candidates.iter().any(|candidate| {
                candidate.path.as_str() == canonical_sensitive.to_string_lossy().as_ref()
                    && candidate.reason.contains("taxpayer/save/final-copy")
            }),
            "{:#?}",
            report.rejected_candidates
        );
        assert!(report.rejected_candidates.iter().any(|candidate| {
            candidate.path.as_str() == canonical_profile.to_string_lossy().as_ref()
                && candidate.reason.contains("taxpayer/save/live-database")
        }));
        if symlink.exists() {
            let canonical_symlink_entry = canonical_input.join("b-link.bin");
            assert!(report.rejected_candidates.iter().any(|candidate| {
                candidate.path.as_str() == canonical_symlink_entry.to_string_lossy().as_ref()
                    && candidate.reason.contains("symlink/reparse")
            }));
        }
    }

    #[test]
    fn search_and_output_paths_must_remain_external() {
        let fixture = Fixture::new("external-only", true);
        let internal_search = fixture.repo_root.join("rules");
        let error = discover_evidence_vault_sources(&fixture.options(
            "source-map.json",
            vec![internal_search],
            true,
        ))
        .expect_err("repository search root must fail");
        assert!(error.to_string().contains("outside repository"), "{error}");

        let internal_output = fixture.repo_root.join("source-map.json");
        let options = DiscoverEvidenceVaultSourcesOptions::new(&fixture.repo_root, internal_output);
        let error =
            discover_evidence_vault_sources(&options).expect_err("repository output must fail");
        assert!(
            error.to_string().contains("outside repository")
                || error.to_string().contains("remain outside repository"),
            "{error}"
        );
    }

    #[test]
    fn manifest_count_is_exactly_43() {
        let fixture = Fixture::new("manifest-count", true);
        fs::remove_dir_all(fixture.repo_root.join("rules/forms/form-043"))
            .expect("remove one fixture form");
        let error =
            discover_evidence_vault_sources(&fixture.options("source-map.json", Vec::new(), true))
                .expect_err("42 manifests must fail");
        assert!(error.to_string().contains("exactly 43"), "{error}");
    }

    #[test]
    fn broad_home_and_credential_roots_are_recognized_before_traversal() {
        use super::{
            is_broad_home_search_root, is_filesystem_or_share_root, sensitive_path_reason,
        };
        use std::collections::BTreeSet;

        assert!(is_filesystem_or_share_root(Path::new("/")));
        assert!(!is_filesystem_or_share_root(Path::new("/official-assets")));
        assert!(is_broad_home_search_root(Path::new("/Users/example")));
        assert!(is_broad_home_search_root(Path::new("/home/example")));
        assert!(!is_broad_home_search_root(Path::new(
            "/Users/example/official-assets"
        )));
        assert!(
            sensitive_path_reason(
                Path::new("/Users/example/.ssh/official-package.bin"),
                &BTreeSet::new(),
                &BTreeSet::new(),
            )
            .is_some()
        );
    }

    #[cfg(windows)]
    #[test]
    fn drive_and_unc_roots_are_rejected_without_confusing_subdirectories() {
        use super::{is_disallowed_parallels_home_search_root, is_filesystem_or_share_root};

        let unc = Path::new(r"\\Mac\Home");
        assert!(unc.is_absolute());
        assert!(is_filesystem_or_share_root(unc));
        assert!(!is_filesystem_or_share_root(Path::new(
            r"\\Mac\Home\official-assets"
        )));
        assert!(is_filesystem_or_share_root(Path::new(r"C:\")));
        assert!(!is_filesystem_or_share_root(Path::new(
            r"C:\official-assets"
        )));
        assert!(!is_disallowed_parallels_home_search_root(unc));
        assert!(is_disallowed_parallels_home_search_root(Path::new(
            r"C:\Mac\Home"
        )));
        assert!(!is_disallowed_parallels_home_search_root(Path::new(
            r"C:\Mac\Home\Downloads"
        )));
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
