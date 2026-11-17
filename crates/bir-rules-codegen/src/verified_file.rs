//! Scope-explicit, race-resistant reads of regular files.
//!
//! Callers validate a resolved path, open it, and then compare the opened
//! handle with fresh handles for the revalidated path before exposing bytes.
//! On Windows every such handle denies write and delete sharing and opens the
//! final component without reparse traversal. Every mutable ancestor directory
//! is likewise opened root-to-leaf without reparse traversal and held with
//! delete sharing denied through the read. A valid 128-bit `FILE_ID_INFO`
//! strengthens the comparison when the provider supplies it.
//!
//! External evidence, vault, packet, staging, and other caller-supplied inputs
//! require valid `FILE_ID_INFO` from every live Windows handle. The observed
//! SMB provider returns ERROR_INVALID_PARAMETER for that query, so a narrowly
//! scoped fallback exists only for files mechanically proven to be inside this
//! tracked checkout. That fallback requires every handle to return the same
//! unsupported result and retains the restrictive file and ancestor handles,
//! but it is not a universal replacement proof: Windows POSIX rename semantics
//! can bypass ordinary open-handle replacement locks. Tracked content remains
//! bound by repository digests and audits. Invalid identities, mixed provider
//! capability, and all other query errors fail closed. Neither scope claims
//! that a pre-open path snapshot remained unchanged before the successful open.

use std::fs::{self, File, Metadata};
use std::io;
use std::path::{Path, PathBuf};

#[cfg(not(windows))]
use same_file::Handle;

use crate::error::{CodegenError, Result};

#[cfg(windows)]
const WINDOWS_LINK_COUNT_ATTEMPTS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VerificationScope {
    TrackedRepository,
    External,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(windows), allow(dead_code))]
enum VerifiedPathKind {
    RegularFile,
    Directory,
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedPathIdentity(bir_rules_platform::WindowsFileIdentity);

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedPathIdentity {
    device: u64,
    inode: u64,
}

#[cfg(not(any(unix, windows)))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedPathIdentity {
    length: u64,
    modified: Option<std::time::SystemTime>,
}

#[cfg(windows)]
#[derive(Debug)]
struct VerifiedHandle {
    file: File,
    identity: Option<bir_rules_platform::WindowsFileIdentity>,
    ancestor_handles: Vec<(File, Option<bir_rules_platform::WindowsFileIdentity>)>,
}

#[cfg(windows)]
impl VerifiedHandle {
    fn from_path(path: &Path, scope: VerificationScope) -> io::Result<Self> {
        Self::from_path_kind(path, scope, VerifiedPathKind::RegularFile)
    }

    fn from_directory_path(path: &Path, scope: VerificationScope) -> io::Result<Self> {
        Self::from_path_kind(path, scope, VerifiedPathKind::Directory)
    }

    fn from_path_kind(
        path: &Path,
        scope: VerificationScope,
        expected_kind: VerifiedPathKind,
    ) -> io::Result<Self> {
        use std::os::windows::fs::OpenOptionsExt as _;

        const GENERIC_READ: u32 = 0x8000_0000;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

        // Acquire every mutable ancestor root-to-leaf. Each earlier handle
        // prevents its directory from being renamed while the next component
        // is opened. The filesystem/share root has no parent and is the only
        // excluded, non-renamable ancestor.
        let mut ancestor_paths = path
            .ancestors()
            .skip(1)
            .take_while(|ancestor| ancestor.parent().is_some())
            .collect::<Vec<_>>();
        ancestor_paths.reverse();
        let mut ancestor_handles = Vec::with_capacity(ancestor_paths.len());
        for ancestor in ancestor_paths {
            let directory = fs::OpenOptions::new()
                // Generic read participates in Windows share-access checks;
                // FILE_READ_ATTRIBUTES alone does not prevent a DELETE open.
                .access_mode(GENERIC_READ)
                .share_mode(FILE_SHARE_READ)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                .open(ancestor)
                .map_err(|source| {
                    io::Error::new(
                        source.kind(),
                        format!(
                            "open restrictive Windows ancestor `{}`: {source}",
                            ancestor.display()
                        ),
                    )
                })?;
            let metadata = directory.metadata().map_err(|source| {
                io::Error::new(
                    source.kind(),
                    format!(
                        "inspect restrictive Windows ancestor `{}`: {source}",
                        ancestor.display()
                    ),
                )
            })?;
            if !metadata.is_dir() || is_symlink_or_reparse_point(&metadata) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "restrictive Windows ancestor `{}` is not a real directory",
                        ancestor.display()
                    ),
                ));
            }
            let identity = classify_windows_file_identity(
                scope,
                bir_rules_platform::file_identity(&directory),
            )
            .map_err(|source| {
                io::Error::new(
                    source.kind(),
                    format!(
                        "identify restrictive Windows ancestor `{}`: {source}",
                        ancestor.display()
                    ),
                )
            })?;
            ancestor_handles.push((directory, identity));
        }

        let mut final_options = fs::OpenOptions::new();
        final_options
            .access_mode(GENERIC_READ)
            .share_mode(FILE_SHARE_READ);
        let final_flags = match expected_kind {
            VerifiedPathKind::RegularFile => FILE_FLAG_OPEN_REPARSE_POINT,
            VerifiedPathKind::Directory => {
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT
            }
        };
        final_options.custom_flags(final_flags);
        let file = final_options.open(path)?;
        let metadata = file.metadata()?;
        let kind_matches = match expected_kind {
            VerifiedPathKind::RegularFile => metadata.is_file(),
            VerifiedPathKind::Directory => metadata.is_dir(),
        };
        if !kind_matches || is_symlink_or_reparse_point(&metadata) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("restrictive Windows open did not resolve to a real {expected_kind:?}"),
            ));
        }

        // The observed UNC provider rejects FileIdInfo (class 0x12) with
        // ERROR_INVALID_PARAMETER. Only that explicit unsupported result may
        // select the restrictive-handle fallback. Invalid identities and all
        // other errors fail closed. While a fallback handle lives, later
        // write, delete, final-component replacement, and ancestor rename
        // opens are incompatible because the ancestor handles remain live. A
        // valid identity is an additional check.
        let identity =
            classify_windows_file_identity(scope, bir_rules_platform::file_identity(&file))?;
        let mut live_identities = ancestor_handles
            .iter()
            .map(|(_, identity)| *identity)
            .collect::<Vec<_>>();
        live_identities.push(identity);
        validate_windows_identity_support(scope, &live_identities)?;
        Ok(Self {
            file,
            identity,
            ancestor_handles,
        })
    }

    fn identity_match(&self, other: &Self) -> Option<bool> {
        if self.ancestor_handles.len() != other.ancestor_handles.len() {
            return Some(false);
        }
        let mut aggregate = windows_identity_match(self.identity.as_ref(), other.identity.as_ref());
        for ((_, left), (_, right)) in self.ancestor_handles.iter().zip(&other.ancestor_handles) {
            aggregate = match (
                aggregate,
                windows_identity_match(left.as_ref(), right.as_ref()),
            ) {
                (Some(false), _) | (_, Some(false)) => Some(false),
                (Some(true), Some(true)) => Some(true),
                (None, None) => None,
                // A handle set is classified uniformly at construction. If a
                // later open reports a different support class, fail closed.
                _ => Some(false),
            };
        }
        aggregate
    }

    fn as_file(&self) -> &File {
        &self.file
    }

    fn as_file_mut(&mut self) -> &mut File {
        &mut self.file
    }

    fn identity_snapshot(&self) -> io::Result<VerifiedPathIdentity> {
        self.identity.map(VerifiedPathIdentity).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "strict Windows identity snapshot is unavailable",
            )
        })
    }
}

#[cfg(windows)]
fn windows_identity_match<T: Eq>(left: Option<&T>, right: Option<&T>) -> Option<bool> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left == right),
        (None, None) => None,
        (Some(_), None) | (None, Some(_)) => Some(false),
    }
}

#[cfg(windows)]
fn classify_windows_file_identity(
    scope: VerificationScope,
    identity: io::Result<bir_rules_platform::WindowsFileIdentity>,
) -> io::Result<Option<bir_rules_platform::WindowsFileIdentity>> {
    const ERROR_INVALID_PARAMETER: i32 = 87;

    match identity {
        Ok(identity) => Ok(Some(identity)),
        Err(error)
            if scope == VerificationScope::TrackedRepository
                && error.raw_os_error() == Some(ERROR_INVALID_PARAMETER) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn validate_windows_identity_support<T>(
    scope: VerificationScope,
    identities: &[Option<T>],
) -> io::Result<()> {
    let has_identity = identities.iter().any(Option::is_some);
    let has_fallback = identities.iter().any(Option::is_none);
    if has_identity && has_fallback {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mixed FILE_ID_INFO support across retained Windows path handles",
        ));
    }
    if scope == VerificationScope::External && has_fallback {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "external Windows path has a retained handle without FILE_ID_INFO",
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
#[derive(Debug)]
struct VerifiedHandle {
    handle: Handle,
}

#[cfg(not(windows))]
impl VerifiedHandle {
    fn from_path(path: &Path, _scope: VerificationScope) -> io::Result<Self> {
        Handle::from_path(path).map(|handle| Self { handle })
    }

    fn from_directory_path(path: &Path, _scope: VerificationScope) -> io::Result<Self> {
        Handle::from_path(path).map(|handle| Self { handle })
    }

    fn identity_match(&self, other: &Self) -> Option<bool> {
        Some(self.handle == other.handle)
    }

    fn as_file(&self) -> &File {
        self.handle.as_file()
    }

    fn as_file_mut(&mut self) -> &mut File {
        self.handle.as_file_mut()
    }

    #[cfg(unix)]
    fn identity_snapshot(&self) -> io::Result<VerifiedPathIdentity> {
        use std::os::unix::fs::MetadataExt as _;

        let metadata = self.handle.as_file().metadata()?;
        Ok(VerifiedPathIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    #[cfg(not(unix))]
    fn identity_snapshot(&self) -> io::Result<VerifiedPathIdentity> {
        let metadata = self.handle.as_file().metadata()?;
        Ok(VerifiedPathIdentity {
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}

#[derive(Debug)]
pub(crate) struct VerifiedRegularFile {
    canonical_path: PathBuf,
    handle: VerifiedHandle,
}

impl VerifiedRegularFile {
    pub(crate) fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    pub(crate) fn file(&self) -> &File {
        self.handle.as_file()
    }

    pub(crate) fn file_mut(&mut self) -> &mut File {
        self.handle.as_file_mut()
    }

    pub(crate) fn identity_snapshot(&self) -> io::Result<VerifiedPathIdentity> {
        self.handle.identity_snapshot()
    }

    pub(crate) fn revalidate_external_path(&self, label: &str) -> Result<()> {
        let canonical = canonical_real_regular_file(&self.canonical_path, label)?;
        if !same_canonical_path(&self.canonical_path, &canonical) {
            return Err(CodegenError::new(format!(
                "{label} path changed after approval (approved `{}`, current `{}`)",
                self.canonical_path.display(),
                canonical.display()
            )));
        }
        let current = VerifiedHandle::from_path(&canonical, VerificationScope::External).map_err(
            |source| {
                CodegenError::io(
                    &format!("revalidate approved {label} identity"),
                    &canonical,
                    source,
                )
            },
        )?;
        if !matches!(self.handle.identity_match(&current), Some(true)) {
            return Err(CodegenError::new(format!(
                "{label} `{}` no longer has its approved identity",
                canonical.display()
            )));
        }
        let metadata = self.handle.as_file().metadata().map_err(|source| {
            CodegenError::io(&format!("reinspect approved {label}"), &canonical, source)
        })?;
        reject_hard_link_alias(
            &self.handle,
            &metadata,
            &canonical,
            label,
            VerificationScope::External,
        )
    }
}

#[derive(Debug)]
pub(crate) struct VerifiedExternalDirectory {
    canonical_path: PathBuf,
    handle: VerifiedHandle,
}

impl VerifiedExternalDirectory {
    pub(crate) fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    pub(crate) fn identity_snapshot(&self) -> io::Result<VerifiedPathIdentity> {
        self.handle.identity_snapshot()
    }

    pub(crate) fn revalidate(&self, label: &str) -> Result<()> {
        let canonical = canonical_real_directory(&self.canonical_path, label)?;
        if !same_canonical_path(&self.canonical_path, &canonical) {
            return Err(CodegenError::new(format!(
                "{label} root changed after approval (approved `{}`, current `{}`)",
                self.canonical_path.display(),
                canonical.display()
            )));
        }
        let current = VerifiedHandle::from_directory_path(&canonical, VerificationScope::External)
            .map_err(|source| {
                CodegenError::io(
                    &format!("revalidate approved {label} root identity"),
                    &canonical,
                    source,
                )
            })?;
        if !matches!(self.handle.identity_match(&current), Some(true)) {
            return Err(CodegenError::new(format!(
                "{label} root `{}` no longer has its approved identity",
                canonical.display()
            )));
        }
        Ok(())
    }
}

/// Open a regular file that is mechanically contained by this tracked
/// checkout.
///
/// On a Windows provider that explicitly rejects FILE_ID_INFO with error 87,
/// this scope alone may use retained restrictive handles plus repository
/// digest/audit integrity. Caller validation is additive and cannot weaken the
/// checkout-containment proof.
pub(crate) fn open_verified_tracked_regular_file<F>(
    path: &Path,
    label: &str,
    validate_resolved_path: F,
) -> Result<VerifiedRegularFile>
where
    F: Fn(&Path) -> Result<()>,
{
    let checkout_root = tracked_checkout_root()?;
    open_verified_regular_file(
        path,
        label,
        VerificationScope::TrackedRepository,
        |resolved| {
            crate::path::ensure_under(
                &checkout_root,
                resolved,
                &format!("resolved tracked {label}"),
            )?;
            validate_resolved_path(resolved)
        },
    )
}

/// Open caller-supplied or otherwise external input with strict identity.
///
/// Windows FILE_ID_INFO must be valid on every live handle. Provider error 87
/// is fatal in this scope even when the path happens to lie inside the checkout.
pub(crate) fn open_verified_external_regular_file<F>(
    path: &Path,
    label: &str,
    validate_resolved_path: F,
) -> Result<VerifiedRegularFile>
where
    F: Fn(&Path) -> Result<()>,
{
    open_verified_regular_file(
        path,
        label,
        VerificationScope::External,
        validate_resolved_path,
    )
}

pub(crate) fn open_verified_external_directory<F>(
    path: &Path,
    label: &str,
    validate_resolved_path: F,
) -> Result<VerifiedExternalDirectory>
where
    F: Fn(&Path) -> Result<()>,
{
    let canonical_before = canonical_real_directory(path, label)?;
    validate_resolved_path(&canonical_before)?;
    let opened_handle =
        VerifiedHandle::from_directory_path(&canonical_before, VerificationScope::External)
            .map_err(|source| {
                CodegenError::io(
                    &format!("open restrictive {label} root"),
                    &canonical_before,
                    source,
                )
            })?;
    let canonical_after = canonical_real_directory(&canonical_before, label)?;
    validate_resolved_path(&canonical_after)?;
    if !same_canonical_path(&canonical_before, &canonical_after) {
        return Err(CodegenError::new(format!(
            "{label} root changed while it was being opened (before `{}`, after `{}`)",
            canonical_before.display(),
            canonical_after.display()
        )));
    }
    let current_handle =
        VerifiedHandle::from_directory_path(&canonical_after, VerificationScope::External)
            .map_err(|source| {
                CodegenError::io(
                    &format!("identify post-open {label} root"),
                    &canonical_after,
                    source,
                )
            })?;
    if !matches!(opened_handle.identity_match(&current_handle), Some(true)) {
        return Err(CodegenError::new(format!(
            "{label} root `{}` was replaced while it was being opened",
            canonical_after.display()
        )));
    }
    let canonical_final = canonical_real_directory(&canonical_after, label)?;
    validate_resolved_path(&canonical_final)?;
    if !same_canonical_path(&canonical_before, &canonical_final) {
        return Err(CodegenError::new(format!(
            "{label} root changed during identity verification (before `{}`, final `{}`)",
            canonical_before.display(),
            canonical_final.display()
        )));
    }
    let final_handle =
        VerifiedHandle::from_directory_path(&canonical_final, VerificationScope::External)
            .map_err(|source| {
                CodegenError::io(
                    &format!("identify final {label} root"),
                    &canonical_final,
                    source,
                )
            })?;
    if !matches!(opened_handle.identity_match(&final_handle), Some(true)) {
        return Err(CodegenError::new(format!(
            "{label} root `{}` was replaced during identity verification",
            canonical_final.display()
        )));
    }
    Ok(VerifiedExternalDirectory {
        canonical_path: canonical_before,
        handle: opened_handle,
    })
}

fn tracked_checkout_root() -> Result<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let checkout_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("bir-rules-codegen is nested under the checkout crates directory");
    let canonical = fs::canonicalize(checkout_root).map_err(|source| {
        CodegenError::io("canonicalize tracked checkout root", checkout_root, source)
    })?;
    let metadata = fs::symlink_metadata(&canonical)
        .map_err(|source| CodegenError::io("inspect tracked checkout root", &canonical, source))?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "tracked checkout root `{}` must be a real directory",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn open_verified_regular_file<F>(
    path: &Path,
    label: &str,
    scope: VerificationScope,
    validate_resolved_path: F,
) -> Result<VerifiedRegularFile>
where
    F: Fn(&Path) -> Result<()>,
{
    let canonical_before = canonical_real_regular_file(path, label)?;
    validate_resolved_path(&canonical_before)?;

    // Opening can race with a same-user path replacement. Do not read from the
    // handle until the post-open path and file identity checks below pass.
    let opened_handle = VerifiedHandle::from_path(&canonical_before, scope).map_err(|source| {
        CodegenError::io(
            &format!("open restrictive {label}"),
            &canonical_before,
            source,
        )
    })?;
    let opened_metadata = opened_handle.as_file().metadata().map_err(|source| {
        CodegenError::io(
            &format!("inspect opened {label}"),
            &canonical_before,
            source,
        )
    })?;
    if !opened_metadata.is_file() || is_symlink_or_reparse_point(&opened_metadata) {
        return Err(CodegenError::new(format!(
            "opened {label} `{}` is not a real regular file",
            canonical_before.display()
        )));
    }

    let canonical_after = canonical_real_regular_file(&canonical_before, label)?;
    validate_resolved_path(&canonical_after)?;
    if !same_canonical_path(&canonical_before, &canonical_after) {
        return Err(CodegenError::new(format!(
            "{label} path changed while it was being opened (before `{}`, after `{}`)",
            canonical_before.display(),
            canonical_after.display()
        )));
    }

    let current_handle = VerifiedHandle::from_path(&canonical_after, scope).map_err(|source| {
        CodegenError::io(
            &format!("identify post-open {label}"),
            &canonical_after,
            source,
        )
    })?;
    if matches!(opened_handle.identity_match(&current_handle), Some(false)) {
        return Err(CodegenError::new(format!(
            "{label} `{}` was replaced while it was being opened; refusing to read",
            canonical_after.display()
        )));
    }

    // A final resolution check closes the window between the second
    // canonicalization and the fresh restrictive open. Once it passes, the
    // still-live opened handle prevents later replacement from redirecting
    // reads. When both handles have FILE_ID_INFO, a mismatch also rejects.
    let canonical_final = canonical_real_regular_file(&canonical_after, label)?;
    validate_resolved_path(&canonical_final)?;
    if !same_canonical_path(&canonical_before, &canonical_final) {
        return Err(CodegenError::new(format!(
            "{label} path changed during identity verification (before `{}`, final `{}`)",
            canonical_before.display(),
            canonical_final.display()
        )));
    }
    let final_handle = VerifiedHandle::from_path(&canonical_final, scope).map_err(|source| {
        CodegenError::io(&format!("identify final {label}"), &canonical_final, source)
    })?;
    if matches!(opened_handle.identity_match(&final_handle), Some(false)) {
        return Err(CodegenError::new(format!(
            "{label} `{}` was replaced during identity verification; refusing to read",
            canonical_final.display()
        )));
    }
    let final_metadata = opened_handle.as_file().metadata().map_err(|source| {
        CodegenError::io(
            &format!("reinspect opened {label}"),
            &canonical_final,
            source,
        )
    })?;
    reject_hard_link_alias(
        &opened_handle,
        &final_metadata,
        &canonical_final,
        label,
        scope,
    )?;

    Ok(VerifiedRegularFile {
        canonical_path: canonical_before,
        handle: opened_handle,
    })
}

fn canonical_real_regular_file(path: &Path, label: &str) -> Result<PathBuf> {
    reject_symlink_ancestors(path, label)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| CodegenError::io(&format!("inspect {label}"), path, source))?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_file() {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be a real regular file, not a symlink/reparse point",
            path.display()
        )));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label}"), path, source))?;
    reject_symlink_ancestors(&canonical, label)?;
    let canonical_metadata = fs::symlink_metadata(&canonical).map_err(|source| {
        CodegenError::io(&format!("inspect canonical {label}"), &canonical, source)
    })?;
    if is_symlink_or_reparse_point(&canonical_metadata) || !canonical_metadata.is_file() {
        return Err(CodegenError::new(format!(
            "canonical {label} `{}` must remain a real regular file",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn canonical_real_directory(path: &Path, label: &str) -> Result<PathBuf> {
    reject_symlink_ancestors(path, label)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| CodegenError::io(&format!("inspect {label} root"), path, source))?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "{label} root `{}` must be a real directory, not a symlink/reparse point",
            path.display()
        )));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label} root"), path, source))?;
    reject_symlink_ancestors(&canonical, label)?;
    let canonical_metadata = fs::symlink_metadata(&canonical).map_err(|source| {
        CodegenError::io(
            &format!("inspect canonical {label} root"),
            &canonical,
            source,
        )
    })?;
    if is_symlink_or_reparse_point(&canonical_metadata) || !canonical_metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "canonical {label} root `{}` must remain a real directory",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn reject_symlink_ancestors(path: &Path, label: &str) -> Result<()> {
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|source| {
            CodegenError::io(&format!("inspect {label} ancestor"), ancestor, source)
        })?;
        if is_symlink_or_reparse_point(&metadata) {
            return Err(CodegenError::new(format!(
                "{label} ancestor `{}` is a symlink/reparse point",
                ancestor.display()
            )));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_symlink_or_reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_symlink_or_reparse_point(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn same_canonical_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(not(windows))]
fn same_canonical_path(left: &Path, right: &Path) -> bool {
    left == right
}

#[cfg(unix)]
fn reject_hard_link_alias(
    _handle: &VerifiedHandle,
    metadata: &Metadata,
    path: &Path,
    label: &str,
    _scope: VerificationScope,
) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    if metadata.nlink() != 1 {
        return Err(CodegenError::new(format!(
            "{label} `{}` has {} hard links; aliased external inputs are forbidden",
            path.display(),
            metadata.nlink()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn reject_hard_link_alias(
    handle: &VerifiedHandle,
    _metadata: &Metadata,
    path: &Path,
    label: &str,
    scope: VerificationScope,
) -> Result<()> {
    let link_count = stable_windows_legacy_link_count(handle.as_file(), path, label)?;
    verify_windows_hard_link_evidence(
        handle,
        path,
        label,
        link_count,
        || bir_rules_platform::standard_link_count(handle.as_file()),
        || std::thread::sleep(std::time::Duration::from_millis(1)),
        scope,
    )
}

#[cfg(windows)]
fn verify_windows_hard_link_evidence(
    opened_handle: &VerifiedHandle,
    path: &Path,
    label: &str,
    legacy_link_count: u64,
    query_standard_count: impl FnMut() -> io::Result<u64>,
    wait: impl FnMut(),
    scope: VerificationScope,
) -> Result<()> {
    if legacy_link_count > 1 {
        return Err(CodegenError::new(format!(
            "{label} `{}` has {legacy_link_count} legacy hard links; aliased external inputs are forbidden",
            path.display()
        )));
    }

    // The legacy BY_HANDLE_FILE_INFORMATION count is never sufficient for
    // acceptance. FILE_STANDARD_INFO is queried on the already-opened handle
    // for both legacy zero and legacy one. Identity and real-path checks on
    // both sides bracket the mandatory independent count.
    revalidate_windows_alias_path(opened_handle, path, label, "before", scope)?;
    let standard_count =
        stable_windows_standard_link_count_with(query_standard_count, wait, path, label)?;
    revalidate_windows_alias_path(opened_handle, path, label, "after", scope)?;

    if standard_count != 1 {
        return Err(CodegenError::new(format!(
            "{label} `{}` reported {legacy_link_count} legacy hard links and {standard_count} FILE_STANDARD_INFO hard links; aliased external inputs are forbidden",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn revalidate_windows_alias_path(
    opened_handle: &VerifiedHandle,
    path: &Path,
    label: &str,
    stage: &str,
    scope: VerificationScope,
) -> Result<()> {
    let canonical = canonical_real_regular_file(path, label)?;
    if !same_canonical_path(path, &canonical) {
        return Err(CodegenError::new(format!(
            "{label} path changed {stage} Windows alias verification (expected `{}`, resolved `{}`)",
            path.display(),
            canonical.display()
        )));
    }
    let current_handle = VerifiedHandle::from_path(&canonical, scope).map_err(|source| {
        CodegenError::io(
            &format!("identify {label} {stage} Windows alias verification"),
            &canonical,
            source,
        )
    })?;
    if matches!(opened_handle.identity_match(&current_handle), Some(false)) {
        return Err(CodegenError::new(format!(
            "{label} `{}` was replaced {stage} Windows alias verification; refusing to read",
            canonical.display()
        )));
    }
    Ok(())
}

/// Obtain a trustworthy Windows hard-link count from one already-opened file.
///
/// A legacy count greater than one is sufficient to reject. Every possible
/// single-link result is instead taken from the independently queried
/// FILE_STANDARD_INFO count; zero and errors exhaust bounded retries and fail
/// closed. Callers therefore cannot accept solely from legacy handle data.
#[cfg(windows)]
pub(crate) fn stable_windows_link_count(file: &File, path: &Path, label: &str) -> Result<u64> {
    let legacy_link_count = stable_windows_legacy_link_count(file, path, label)?;
    if legacy_link_count > 1 {
        return Ok(legacy_link_count);
    }
    stable_windows_standard_link_count_with(
        || bir_rules_platform::standard_link_count(file),
        || std::thread::sleep(std::time::Duration::from_millis(1)),
        path,
        label,
    )
}

/// Obtain the advisory legacy Windows hard-link count from one open file.
///
/// Some shared-filesystem redirectors transiently return the invalid value
/// zero for `nNumberOfLinks` under load. Zero is never accepted as safe: retry
/// the same handle with a one-millisecond bounded backoff. A persistent zero is
/// returned to the caller only as a signal for the mandatory independent
/// FILE_STANDARD_INFO check; it is never accepted as a single-link result. The
/// check occurs once at the final identity boundary, so persistent zero adds at
/// most 31 milliseconds per opened file rather than multiplying across the
/// read path.
#[cfg(windows)]
fn stable_windows_legacy_link_count(file: &File, path: &Path, label: &str) -> Result<u64> {
    stable_windows_legacy_link_count_with(
        || winapi_util::file::information(file).map(|information| information.number_of_links()),
        || std::thread::sleep(std::time::Duration::from_millis(1)),
        path,
        label,
    )
}

#[cfg(windows)]
fn stable_windows_legacy_link_count_with(
    query: impl FnMut() -> std::io::Result<u64>,
    wait: impl FnMut(),
    path: &Path,
    label: &str,
) -> Result<u64> {
    retry_zero_windows_link_count(query, wait)
        .map_err(|source| CodegenError::io(&format!("inspect {label} link count"), path, source))
}

#[cfg(windows)]
fn retry_zero_windows_link_count(
    mut query: impl FnMut() -> std::io::Result<u64>,
    mut wait: impl FnMut(),
) -> std::io::Result<u64> {
    for attempt in 0..WINDOWS_LINK_COUNT_ATTEMPTS {
        let link_count = query()?;
        if link_count != 0 || attempt + 1 == WINDOWS_LINK_COUNT_ATTEMPTS {
            return Ok(link_count);
        }
        wait();
    }
    unreachable!("the bounded link-count loop always returns")
}

/// Obtain a nonzero `FILE_STANDARD_INFO` count from the already-opened handle.
///
/// Both zero results and query errors can be transient on SMB redirectors, so
/// they are retried with the same bounded delay as the legacy query. Exhausting
/// the bound is an error carrying the observation counts and last OS error.
#[cfg(windows)]
fn stable_windows_standard_link_count_with(
    query: impl FnMut() -> io::Result<u64>,
    wait: impl FnMut(),
    path: &Path,
    label: &str,
) -> Result<u64> {
    retry_windows_standard_link_count(query, wait).map_err(|source| {
        CodegenError::io(
            &format!("inspect {label} FILE_STANDARD_INFO link count"),
            path,
            source,
        )
    })
}

#[cfg(windows)]
fn retry_windows_standard_link_count(
    mut query: impl FnMut() -> io::Result<u64>,
    mut wait: impl FnMut(),
) -> io::Result<u64> {
    let mut zero_results = 0_usize;
    let mut error_results = 0_usize;
    let mut last_error = None;

    for attempt in 0..WINDOWS_LINK_COUNT_ATTEMPTS {
        match query() {
            Ok(link_count) if link_count != 0 => return Ok(link_count),
            Ok(0) => zero_results += 1,
            Ok(_) => unreachable!("u64 link counts are zero or greater than zero"),
            Err(error) => {
                error_results += 1;
                last_error = Some(error);
            }
        }
        if attempt + 1 != WINDOWS_LINK_COUNT_ATTEMPTS {
            wait();
        }
    }

    let last_error = last_error
        .as_ref()
        .map_or_else(|| "no OS error".to_owned(), |error| error.to_string());
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "FILE_STANDARD_INFO did not return a nonzero hard-link count after {WINDOWS_LINK_COUNT_ATTEMPTS} attempts ({zero_results} zero results, {error_results} errors; last error: {last_error})"
        ),
    ))
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;
    use std::fs;
    use std::io::Read as _;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[cfg(windows)]
    #[test]
    fn zero_legacy_windows_link_counts_are_retried_for_the_mandatory_standard_check() {
        let mut observations = [0_u64, 0, 1].into_iter();
        let mut waits = 0_usize;
        let recovered = retry_zero_windows_link_count(
            || Ok(observations.next().expect("bounded observation")),
            || waits += 1,
        )
        .expect("synthetic query");
        assert_eq!(recovered, 1);
        assert_eq!(waits, 2);

        let mut attempts = 0_usize;
        let unresolved = retry_zero_windows_link_count(
            || {
                attempts += 1;
                Ok(0)
            },
            || {},
        )
        .expect("synthetic query");
        assert_eq!(unresolved, 0);
        assert_eq!(attempts, WINDOWS_LINK_COUNT_ATTEMPTS);

        let fallback_signal = stable_windows_legacy_link_count_with(
            || Ok(0),
            || {},
            Path::new("synthetic-file"),
            "test asset",
        )
        .expect("persistent zero is preserved only for the independent standard query");
        assert_eq!(fallback_signal, 0);
    }

    #[cfg(windows)]
    #[test]
    fn standard_info_retries_zero_and_errors_but_exhaustion_fails_closed() {
        let mut observations = [
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "transient SMB query error",
            )),
            Ok(0),
            Ok(1),
        ]
        .into_iter();
        let mut waits = 0_usize;
        let recovered = retry_windows_standard_link_count(
            || observations.next().expect("bounded observation"),
            || waits += 1,
        )
        .expect("transient standard-info observations recover");
        assert_eq!(recovered, 1);
        assert_eq!(waits, 2);

        let persistent_zero =
            retry_windows_standard_link_count(|| Ok(0), || {}).expect_err("zero must exhaust");
        let persistent_zero = persistent_zero.to_string();
        assert!(persistent_zero.contains("after 32 attempts"));
        assert!(persistent_zero.contains("32 zero results, 0 errors"));

        let persistent_error = retry_windows_standard_link_count(
            || {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "provider has no FILE_STANDARD_INFO",
                ))
            },
            || {},
        )
        .expect_err("errors must exhaust");
        let persistent_error = persistent_error.to_string();
        assert!(persistent_error.contains("after 32 attempts"));
        assert!(persistent_error.contains("0 zero results, 32 errors"));
        assert!(persistent_error.contains("provider has no FILE_STANDARD_INFO"));
    }

    #[cfg(windows)]
    #[test]
    fn file_identity_fallback_accepts_only_explicit_provider_unsupported() {
        assert!(
            classify_windows_file_identity(
                VerificationScope::TrackedRepository,
                Err(io::Error::from_raw_os_error(87)),
            )
            .expect("ERROR_INVALID_PARAMETER selects the restrictive-handle fallback")
            .is_none()
        );
        let strict_unsupported = classify_windows_file_identity(
            VerificationScope::External,
            Err(io::Error::from_raw_os_error(87)),
        )
        .expect_err("strict external identity rejects ERROR_INVALID_PARAMETER");
        assert_eq!(strict_unsupported.raw_os_error(), Some(87));

        let invalid = classify_windows_file_identity(
            VerificationScope::External,
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "all-zero FILE_ID_INFO",
            )),
        )
        .expect_err("invalid identity data must fail closed");
        assert_eq!(invalid.kind(), io::ErrorKind::InvalidData);

        let denied = classify_windows_file_identity(
            VerificationScope::External,
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "identity query denied",
            )),
        )
        .expect_err("arbitrary identity errors must fail closed");
        assert_eq!(denied.kind(), io::ErrorKind::PermissionDenied);

        let generic_unsupported = classify_windows_file_identity(
            VerificationScope::External,
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "unclassified provider error",
            )),
        )
        .expect_err("unclassified unsupported errors must fail closed");
        assert_eq!(generic_unsupported.kind(), io::ErrorKind::Unsupported);
    }

    #[cfg(windows)]
    #[test]
    fn mixed_file_id_support_fails_closed() {
        assert_eq!(windows_identity_match(Some(&1_u8), Some(&1)), Some(true));
        assert_eq!(windows_identity_match(Some(&1_u8), Some(&2)), Some(false));
        assert_eq!(windows_identity_match::<u8>(None, None), None);
        assert_eq!(windows_identity_match(Some(&1_u8), None), Some(false));
        assert_eq!(windows_identity_match(None, Some(&1_u8)), Some(false));
        validate_windows_identity_support(
            VerificationScope::TrackedRepository,
            &[Some(1_u8), Some(2_u8)],
        )
        .expect("uniform tracked FILE_ID_INFO support");
        validate_windows_identity_support::<u8>(
            VerificationScope::TrackedRepository,
            &[None, None],
        )
        .expect("uniform tracked restrictive-handle fallback");
        let mixed = validate_windows_identity_support(
            VerificationScope::TrackedRepository,
            &[Some(1_u8), None],
        )
        .expect_err("mixed tracked identity support must fail");
        assert!(mixed.to_string().contains("mixed FILE_ID_INFO support"));
        validate_windows_identity_support::<u8>(VerificationScope::External, &[None])
            .expect_err("external handles may never select fallback");
    }

    fn temp_root(label: &str) -> PathBuf {
        let root = crate::test_temp_dir().join(format!(
            "bir-rules-codegen-verified-file-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn tracked_opener_rejects_files_outside_the_checkout() {
        let root = temp_root("tracked-containment");
        let path = root.join("external.bin");
        fs::write(&path, b"external-bytes").unwrap();

        let error =
            open_verified_tracked_regular_file(&path, "tracked containment test", |_| Ok(()))
                .expect_err("tracked reads must be mechanically contained by the checkout");
        assert!(error.to_string().contains("escapes repository root"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verified_file_is_unread_until_path_and_handle_identity_pass() {
        let root = temp_root("identity");
        let path = root.join("asset.bin");
        fs::write(&path, b"official-bytes").unwrap();
        let calls = AtomicUsize::new(0);

        let mut verified = open_verified_external_regular_file(&path, "test asset", |resolved| {
            assert!(resolved.is_absolute());
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert_eq!(verified.canonical_path(), fs::canonicalize(&path).unwrap());
        let mut bytes = Vec::new();
        verified.file_mut().read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"official-bytes");
        assert!(verified.file().metadata().unwrap().is_file());
        drop(verified);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn replacement_after_open_is_rejected_before_any_read() {
        let root = temp_root("replace");
        let path = root.join("asset.bin");
        let replacement = root.join("replacement.bin");
        fs::write(&path, b"official-bytes").unwrap();
        fs::write(&replacement, b"private-bytes!").unwrap();
        let calls = AtomicUsize::new(0);

        let error = open_verified_external_regular_file(&path, "test asset", |_| {
            if calls.fetch_add(1, Ordering::SeqCst) == 1 {
                fs::rename(&replacement, &path).unwrap();
            }
            Ok(())
        })
        .unwrap_err();
        assert!(error.to_string().contains("replaced"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn hard_link_alias_is_rejected_before_any_read() {
        let root = temp_root("hard-link");
        let path = root.join("asset.bin");
        let alias = root.join("alias.bin");
        fs::write(&path, b"official-bytes").unwrap();
        fs::hard_link(&path, &alias).unwrap();

        let error =
            open_verified_external_regular_file(&alias, "test asset", |_| Ok(())).unwrap_err();
        assert!(error.to_string().contains("hard links"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_symlink_alias_is_rejected_before_any_read() {
        let root = temp_root("symlink");
        let path = root.join("asset.bin");
        let alias = root.join("alias.bin");
        fs::write(&path, b"official-bytes").unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        #[cfg(windows)]
        if let Err(error) = std::os::windows::fs::symlink_file(&path, &alias) {
            const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;
            if error.kind() == io::ErrorKind::PermissionDenied
                || error.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD)
            {
                eprintln!(
                    "skipping symlink-alias exercise because this Windows process cannot create a test symlink: {error}"
                );
                fs::remove_dir_all(root).unwrap();
                return;
            }
            panic!("create test symlink: {error}");
        }

        let error =
            open_verified_external_regular_file(&alias, "test asset", |_| Ok(())).unwrap_err();
        assert!(error.to_string().contains("symlink/reparse point"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn restrictive_checkout_handles_block_mutation_and_path_vacation() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crate directory is nested under the workspace crates directory");
        let root = workspace_root.join("target").join(format!(
            "bir-rules-codegen-restrictive-handle-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();

        let write_path = root.join("write.bin");
        fs::write(&write_path, b"write-protected").unwrap();
        let write_handle =
            open_verified_tracked_regular_file(&write_path, "write lock test", |_| Ok(())).unwrap();
        let write_blocked = fs::OpenOptions::new()
            .write(true)
            .open(&write_path)
            .is_err();
        drop(write_handle);
        let write_control_succeeded = fs::OpenOptions::new().write(true).open(&write_path).is_ok();

        let rename_path = root.join("rename.bin");
        let renamed_path = root.join("renamed.bin");
        fs::write(&rename_path, b"rename-protected").unwrap();
        let rename_handle =
            open_verified_tracked_regular_file(&rename_path, "rename lock test", |_| Ok(()))
                .unwrap();
        let final_rename_blocked = fs::rename(&rename_path, &renamed_path).is_err();
        drop(rename_handle);
        let final_rename_control_succeeded = fs::rename(&rename_path, &renamed_path).is_ok();

        let delete_path = root.join("delete.bin");
        fs::write(&delete_path, b"delete-protected").unwrap();
        let delete_handle =
            open_verified_tracked_regular_file(&delete_path, "delete lock test", |_| Ok(()))
                .unwrap();
        let final_delete_blocked = fs::remove_file(&delete_path).is_err();
        drop(delete_handle);
        let final_delete_control_succeeded = fs::remove_file(&delete_path).is_ok();

        let outer_ancestor = root.join("outer-ancestor");
        let renamed_outer_ancestor = root.join("renamed-outer-ancestor");
        let inner_ancestor = outer_ancestor.join("inner-ancestor");
        let renamed_inner_ancestor = outer_ancestor.join("renamed-inner-ancestor");
        fs::create_dir_all(&inner_ancestor).unwrap();
        let descendant = inner_ancestor.join("descendant.bin");
        fs::write(&descendant, b"ancestor-protected").unwrap();
        let descendant_handle =
            open_verified_tracked_regular_file(&descendant, "ancestor lock test", |_| Ok(()))
                .unwrap();
        let inner_ancestor_rename_blocked =
            fs::rename(&inner_ancestor, &renamed_inner_ancestor).is_err();
        let outer_ancestor_rename_blocked =
            fs::rename(&outer_ancestor, &renamed_outer_ancestor).is_err();
        let descendant_still_current =
            fs::read(&descendant).is_ok_and(|bytes| bytes == b"ancestor-protected");
        drop(descendant_handle);
        let inner_ancestor_control_succeeded =
            fs::rename(&inner_ancestor, &renamed_inner_ancestor).is_ok();
        let outer_ancestor_control_succeeded =
            fs::rename(&outer_ancestor, &renamed_outer_ancestor).is_ok();

        eprintln!(
            "restrictive-handle observation: path=`{}`, canonical=`{}`, write_blocked={write_blocked}, write_control_succeeded={write_control_succeeded}, final_rename_blocked={final_rename_blocked}, final_rename_control_succeeded={final_rename_control_succeeded}, final_delete_blocked={final_delete_blocked}, final_delete_control_succeeded={final_delete_control_succeeded}, inner_ancestor_rename_blocked={inner_ancestor_rename_blocked}, inner_ancestor_control_succeeded={inner_ancestor_control_succeeded}, outer_ancestor_rename_blocked={outer_ancestor_rename_blocked}, outer_ancestor_control_succeeded={outer_ancestor_control_succeeded}",
            root.display(),
            canonical_root.display()
        );
        let final_inner_ancestor = renamed_outer_ancestor.join("renamed-inner-ancestor");
        let final_descendant = final_inner_ancestor.join("descendant.bin");
        fs::remove_file(&write_path).expect("remove write control file");
        fs::remove_file(&renamed_path).expect("remove rename control file");
        fs::remove_file(&final_descendant).expect("remove protected descendant");
        fs::remove_dir(&final_inner_ancestor).expect("remove renamed inner ancestor");
        fs::remove_dir(&renamed_outer_ancestor).expect("remove renamed outer ancestor");
        fs::remove_dir(&root).expect("remove empty restrictive-handle test root");

        assert!(write_blocked, "write sharing must remain denied");
        assert!(
            write_control_succeeded,
            "write-open control must succeed after the restrictive handle drops"
        );
        assert!(final_rename_blocked, "final-component rename must fail");
        assert!(
            final_rename_control_succeeded,
            "final-component rename control must succeed after the restrictive handle drops"
        );
        assert!(final_delete_blocked, "final-component delete must fail");
        assert!(
            final_delete_control_succeeded,
            "final-component delete control must succeed after the restrictive handle drops"
        );
        assert!(
            inner_ancestor_rename_blocked,
            "the immediate ancestor must not be renamed or vacated for replacement"
        );
        assert!(
            outer_ancestor_rename_blocked,
            "every higher mutable ancestor must remain locked too"
        );
        assert!(
            inner_ancestor_control_succeeded,
            "immediate-ancestor rename control must succeed after all handles drop"
        );
        assert!(
            outer_ancestor_control_succeeded,
            "higher-ancestor rename control must succeed after all handles drop"
        );
        assert!(
            descendant_still_current,
            "the protected descendant must remain at the validated path"
        );
    }

    #[cfg(windows)]
    #[test]
    fn standard_info_is_required_and_true_aliases_are_rejected_without_name_enumeration() {
        let root = temp_root("standard-count");
        let ordinary_path = root.join("asset.bin");
        let alias = root.join("alias.bin");
        fs::write(&ordinary_path, b"official-bytes").unwrap();
        let path = fs::canonicalize(&ordinary_path).unwrap();
        let opened_handle = VerifiedHandle::from_path(&path, VerificationScope::External).unwrap();

        let mut standard_queries = 0_usize;
        verify_windows_hard_link_evidence(
            &opened_handle,
            &path,
            "test asset",
            1,
            || {
                standard_queries += 1;
                bir_rules_platform::standard_link_count(opened_handle.as_file())
            },
            || {},
            VerificationScope::External,
        )
        .expect("FILE_STANDARD_INFO verifies an ordinary legacy-one file");
        assert_eq!(
            standard_queries, 1,
            "legacy count one must not bypass FILE_STANDARD_INFO"
        );

        let zero_error = verify_windows_hard_link_evidence(
            &opened_handle,
            &path,
            "test asset",
            0,
            || Ok(0),
            || {},
            VerificationScope::External,
        )
        .expect_err("persistent standard zero must fail closed");
        assert!(
            StdError::source(&zero_error)
                .expect("standard-info exhaustion retains its IO source")
                .to_string()
                .contains("32 zero results, 0 errors")
        );

        let unsupported_error = verify_windows_hard_link_evidence(
            &opened_handle,
            &path,
            "test asset",
            0,
            || {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "synthetic unsupported FILE_STANDARD_INFO",
                ))
            },
            || {},
            VerificationScope::External,
        )
        .expect_err("an unsupported standard query must fail closed");
        assert!(
            unsupported_error
                .to_string()
                .contains("inspect test asset FILE_STANDARD_INFO link count")
        );
        assert!(
            StdError::source(&unsupported_error)
                .expect("unsupported standard-info exhaustion retains its IO source")
                .to_string()
                .contains("32 errors; last error: synthetic unsupported FILE_STANDARD_INFO")
        );

        drop(opened_handle);
        fs::hard_link(&path, &alias).unwrap();
        let aliased_handle = VerifiedHandle::from_path(&path, VerificationScope::External).unwrap();
        let standard_error = verify_windows_hard_link_evidence(
            &aliased_handle,
            &path,
            "test asset",
            0,
            || bir_rules_platform::standard_link_count(aliased_handle.as_file()),
            || {},
            VerificationScope::External,
        )
        .expect_err("FILE_STANDARD_INFO must expose the true alias");
        assert!(
            standard_error
                .to_string()
                .contains("2 FILE_STANDARD_INFO hard links")
        );

        drop(aliased_handle);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn checkout_identity_and_standard_info_work_including_on_unc_smb() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/verified_file.rs");
        let canonical = fs::canonicalize(&path).unwrap();
        let observation_file = File::open(&canonical).unwrap();
        let observed_link_count =
            stable_windows_link_count(&observation_file, &canonical, "verified-file source")
                .unwrap();
        let observed_standard_count =
            bir_rules_platform::standard_link_count(&observation_file).unwrap();
        eprintln!(
            "verified-file checkout observation: path=`{}`, stable_link_count={observed_link_count}, standard_link_count={observed_standard_count}",
            canonical.display()
        );
        drop(observation_file);

        let opened_handle =
            VerifiedHandle::from_path(&canonical, VerificationScope::TrackedRepository).unwrap();
        let reopened_handle =
            VerifiedHandle::from_path(&canonical, VerificationScope::TrackedRepository).unwrap();
        assert!(
            !matches!(opened_handle.identity_match(&reopened_handle), Some(false)),
            "valid FILE_ID_INFO must match two handles for the live checkout path"
        );
        let distinct_path =
            fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/error.rs")).unwrap();
        let distinct_handle =
            VerifiedHandle::from_path(&distinct_path, VerificationScope::TrackedRepository)
                .unwrap();
        if let Some(identity_matches) = opened_handle.identity_match(&distinct_handle) {
            assert!(
                !identity_matches,
                "valid FILE_ID_INFO must distinguish two live checkout files"
            );
        }

        let mut standard_queries = 0_usize;
        verify_windows_hard_link_evidence(
            &opened_handle,
            &canonical,
            "verified-file source",
            observed_link_count,
            || {
                standard_queries += 1;
                bir_rules_platform::standard_link_count(opened_handle.as_file())
            },
            || {},
            VerificationScope::TrackedRepository,
        )
        .expect("the live checkout supports restrictive handles and FILE_STANDARD_INFO");
        assert_eq!(
            standard_queries, 1,
            "the live path must require FILE_STANDARD_INFO even for a legacy count of one"
        );
        drop(distinct_handle);
        drop(reopened_handle);
        drop(opened_handle);

        let mut verified =
            open_verified_tracked_regular_file(&path, "verified-file source", |_| Ok(())).unwrap();
        let mut prefix = [0_u8; 3];
        verified.file_mut().read_exact(&mut prefix).unwrap();
        assert_eq!(&prefix, b"//!");
    }
}
