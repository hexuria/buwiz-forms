//! Filesystem reads at a trust boundary.
//!
//! An approved external file or tree retains restrictive handles and binds
//! every canonical path to a platform file identity before any bytes become
//! observable. A bound tree retains handles for its root, every nested
//! directory, and every regular file, then re-enumerates and revalidates the
//! complete inventory after deterministic double reads.
//!
//! This is deliberately a fail-closed consistency protocol, not an atomic
//! filesystem snapshot. In particular, on Unix a same-user writer that can
//! mutate the same inode between the observed metadata/read passes remains an
//! environmental concurrency assumption. Callers must still bind independently
//! verified size and SHA-256 values (and a tree digest for multi-file inputs);
//! the retained-handle protocol does not replace content authentication.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(any(unix, windows)))]
use std::time::SystemTime;

use same_file::Handle;

use crate::error::{CodegenError, Result};
use crate::path::{
    ensure_under, is_json_file, is_symlink_or_reparse_point, normalized_relative_path,
    reject_symlink_components,
};
use crate::verified_file::{
    VerifiedExternalDirectory, VerifiedPathIdentity, VerifiedRegularFile,
    open_verified_external_directory, open_verified_external_regular_file,
    open_verified_tracked_regular_file,
};

static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
thread_local! {
    static BEFORE_TREE_INSTALL_HOOK: std::cell::RefCell<Option<Box<dyn Fn(&Path)>>> =
        std::cell::RefCell::new(None);
    static BOUND_TREE_READ_HOOK: std::cell::RefCell<
        Option<Box<dyn Fn(&Path, &str, usize)>>
    > = std::cell::RefCell::new(None);
    static CWD_CAPTURE_COUNT: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReadScope {
    Tracked,
    External,
}

#[derive(Debug)]
pub(crate) struct ApprovedExternalFile {
    verified: VerifiedRegularFile,
}

impl ApprovedExternalFile {
    pub(crate) fn capture(
        path: &Path,
        label: &str,
        validate_resolved_path: impl Fn(&Path) -> Result<()>,
    ) -> Result<Self> {
        let path = absolute_lexically_normalized(path, label)?;
        let verified = open_verified_external_regular_file(&path, label, validate_resolved_path)?;
        Ok(Self { verified })
    }

    pub(crate) fn path(&self) -> &Path {
        self.verified.canonical_path()
    }
}

#[derive(Debug)]
pub(crate) struct ApprovedExternalRoot {
    verified: VerifiedExternalDirectory,
}

impl ApprovedExternalRoot {
    pub(crate) fn capture(
        path: &Path,
        label: &str,
        validate_resolved_path: impl Fn(&Path) -> Result<()>,
    ) -> Result<Self> {
        let path = absolute_lexically_normalized(path, label)?;
        let verified = open_verified_external_directory(&path, label, validate_resolved_path)?;
        Ok(Self { verified })
    }

    pub(crate) fn path(&self) -> &Path {
        self.verified.canonical_path()
    }

    pub(crate) fn revalidate(&self, label: &str) -> Result<()> {
        self.verified.revalidate(label)
    }
}

pub(crate) fn read_external_bytes_bound(
    mut approved: ApprovedExternalFile,
    label: &str,
) -> Result<Vec<u8>> {
    read_verified_file_stably(&mut approved.verified, label, None)
}

pub(crate) fn read_external_bytes_under(
    approved_root: &ApprovedExternalRoot,
    path: &Path,
    label: &str,
) -> Result<Vec<u8>> {
    approved_root.revalidate(label)?;
    let path = absolute_lexically_normalized(path, label)?;
    let expected = fs::canonicalize(&path)
        .map_err(|source| CodegenError::io(&format!("canonicalize {label}"), &path, source))?;
    ensure_under(
        approved_root.path(),
        &expected,
        &format!("approved {label} child"),
    )?;
    let mut verified = open_verified_external_regular_file(&expected, label, |resolved| {
        if resolved != expected {
            return Err(CodegenError::new(format!(
                "{label} child `{}` resolved to a different canonical file `{}`",
                expected.display(),
                resolved.display()
            )));
        }
        ensure_under(
            approved_root.path(),
            resolved,
            &format!("approved {label} child"),
        )
    })?;
    let read_result =
        read_verified_file_stably(&mut verified, label, Some((approved_root.path(), 0)));
    let root_result = approved_root.revalidate(label);
    root_result?;
    read_result
}

pub(crate) fn read_external_tree_under(
    approved_parent: &ApprovedExternalRoot,
    child_root: &Path,
    label: &str,
) -> Result<BTreeMap<String, Vec<u8>>> {
    approved_parent.revalidate(label)?;
    let child_root = absolute_lexically_normalized(child_root, label)?;
    let expected = fs::canonicalize(&child_root).map_err(|source| {
        CodegenError::io(
            &format!("canonicalize {label} child root"),
            &child_root,
            source,
        )
    })?;
    ensure_under(
        approved_parent.path(),
        &expected,
        &format!("approved {label} child root"),
    )?;
    let approved_child = ApprovedExternalRoot::capture(&expected, label, |resolved| {
        if resolved != expected {
            return Err(CodegenError::new(format!(
                "{label} child root `{}` resolved to a different canonical directory `{}`",
                expected.display(),
                resolved.display()
            )));
        }
        ensure_under(
            approved_parent.path(),
            resolved,
            &format!("approved {label} child root"),
        )
    })?;
    let read_result = read_external_tree_bound(&approved_child, label);
    let parent_result = approved_parent.revalidate(label);
    parent_result?;
    read_result
}

pub(crate) fn read_external_tree_bound(
    approved: &ApprovedExternalRoot,
    label: &str,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let root = approved.verified.canonical_path();
    approved.verified.revalidate(label)?;
    let inventory_before = collect_bound_tree_inventory(root, label)?;
    let mut opened_directories = Vec::new();
    let mut opened_files = Vec::new();

    // `_index` is consumed by the `#[cfg(test)]` read hooks below, so the
    // index only looks discarded in a non-test build.
    #[allow(clippy::unused_enumerate_index)]
    for (_index, (relative, entry)) in inventory_before.iter().enumerate() {
        let path = root.join(relative);
        let expected = fs::canonicalize(&path).map_err(|source| {
            CodegenError::io(&format!("canonicalize {label} tree entry"), &path, source)
        })?;
        ensure_under(root, &expected, &format!("approved {label} tree entry"))?;

        match entry.kind {
            BoundTreeEntryKind::Directory => {
                let verified = open_verified_external_directory(&expected, label, |resolved| {
                    if resolved != expected {
                        return Err(CodegenError::new(format!(
                            "{label} directory `{}` resolved to a different canonical directory `{}`",
                            expected.display(),
                            resolved.display()
                        )));
                    }
                    ensure_under(root, resolved, &format!("approved {label} directory"))
                })?;
                #[cfg(test)]
                BOUND_TREE_READ_HOOK.with(|hook| {
                    if let Some(hook) = hook.borrow().as_ref() {
                        hook(root, "after-entry-open", _index);
                    }
                });
                require_bound_tree_identity(
                    verified.identity_snapshot(),
                    entry.identity,
                    label,
                    &expected,
                )?;
                opened_directories.push((relative.clone(), verified));
            }
            BoundTreeEntryKind::RegularFile => {
                let verified = open_verified_external_regular_file(&expected, label, |resolved| {
                    if resolved != expected {
                        return Err(CodegenError::new(format!(
                            "{label} file `{}` resolved to a different canonical file `{}`",
                            expected.display(),
                            resolved.display()
                        )));
                    }
                    ensure_under(root, resolved, &format!("approved {label} file"))
                })?;
                #[cfg(test)]
                BOUND_TREE_READ_HOOK.with(|hook| {
                    if let Some(hook) = hook.borrow().as_ref() {
                        hook(root, "after-entry-open", _index);
                    }
                });
                require_bound_tree_identity(
                    verified.identity_snapshot(),
                    entry.identity,
                    label,
                    &expected,
                )?;
                opened_files.push((relative.clone(), verified));
            }
        }

        #[cfg(test)]
        BOUND_TREE_READ_HOOK.with(|hook| {
            if let Some(hook) = hook.borrow().as_ref() {
                hook(root, "after-open", _index);
            }
        });
    }
    #[cfg(test)]
    BOUND_TREE_READ_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow().as_ref() {
            hook(root, "after-all-open", inventory_before.len());
        }
    });

    approved.verified.revalidate(label)?;
    let inventory_after_open = collect_bound_tree_inventory(root, label)?;
    if inventory_after_open != inventory_before {
        return Err(CodegenError::new(format!(
            "{label} root `{}` changed during tree read",
            root.display()
        )));
    }
    for (_, verified) in &opened_directories {
        verified.revalidate(label)?;
    }
    for (_, verified) in &opened_files {
        verified.revalidate_external_path(label)?;
    }

    let mut files = BTreeMap::new();
    for (index, (relative, verified)) in opened_files.iter_mut().enumerate() {
        let bytes = read_verified_file_stably(verified, label, Some((root, index)))?;
        files.insert(relative.clone(), bytes);
    }

    for (_, verified) in &opened_directories {
        verified.revalidate(label)?;
    }
    for (_, verified) in &opened_files {
        verified.revalidate_external_path(label)?;
    }
    approved.verified.revalidate(label)?;
    let inventory_final = collect_bound_tree_inventory(root, label)?;
    if inventory_final != inventory_before {
        return Err(CodegenError::new(format!(
            "{label} root `{}` changed before bound tree completion",
            root.display()
        )));
    }
    Ok(files)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundTreeEntryKind {
    Directory,
    RegularFile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BoundTreeEntry {
    kind: BoundTreeEntryKind,
    identity: VerifiedPathIdentity,
}

#[cfg(unix)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct StableFileMetadata {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

#[cfg(windows)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct StableFileMetadata {
    len: u64,
    creation_time: u64,
    last_write_time: u64,
}

#[cfg(not(any(unix, windows)))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct StableFileMetadata {
    len: u64,
    modified: Option<SystemTime>,
}

fn read_verified_file_stably(
    verified: &mut VerifiedRegularFile,
    label: &str,
    tree_hook: Option<(&Path, usize)>,
) -> Result<Vec<u8>> {
    verified.revalidate_external_path(label)?;
    let path = verified.canonical_path().to_path_buf();
    let metadata_before = stable_file_metadata(verified.file(), label, &path)?;
    verified
        .file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|source| CodegenError::io(&format!("rewind {label} file"), &path, source))?;
    let mut first = Vec::new();
    verified
        .file_mut()
        .read_to_end(&mut first)
        .map_err(|source| CodegenError::io(&format!("read {label} file"), &path, source))?;
    #[cfg(test)]
    if let Some((root, index)) = tree_hook {
        BOUND_TREE_READ_HOOK.with(|hook| {
            if let Some(hook) = hook.borrow().as_ref() {
                hook(root, "after-first-read", index);
            }
        });
    }
    #[cfg(not(test))]
    let _ = tree_hook;
    let metadata_between = stable_file_metadata(verified.file(), label, &path)?;
    verified
        .file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|source| CodegenError::io(&format!("rewind {label} file"), &path, source))?;
    let mut second = Vec::new();
    verified
        .file_mut()
        .read_to_end(&mut second)
        .map_err(|source| CodegenError::io(&format!("reread {label} file"), &path, source))?;
    let metadata_after = stable_file_metadata(verified.file(), label, &path)?;
    if first != second
        || metadata_before != metadata_between
        || metadata_between != metadata_after
        || metadata_after.len != second.len() as u64
    {
        return Err(CodegenError::new(format!(
            "{label} file `{}` changed during stable double-read",
            path.display()
        )));
    }
    verified.revalidate_external_path(label)?;
    Ok(second)
}

fn stable_file_metadata(file: &File, label: &str, path: &Path) -> Result<StableFileMetadata> {
    let metadata = file
        .metadata()
        .map_err(|source| CodegenError::io(&format!("inspect {label} file"), path, source))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        Ok(StableFileMetadata {
            dev: metadata.dev(),
            ino: metadata.ino(),
            len: metadata.len(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        })
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        Ok(StableFileMetadata {
            len: metadata.file_size(),
            creation_time: metadata.creation_time(),
            last_write_time: metadata.last_write_time(),
        })
    }
    #[cfg(not(any(unix, windows)))]
    Ok(StableFileMetadata {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn collect_bound_tree_inventory(
    root: &Path,
    label: &str,
) -> Result<BTreeMap<String, BoundTreeEntry>> {
    let mut inventory = BTreeMap::new();
    collect_bound_tree_entries(root, root, label, &mut inventory)?;
    Ok(inventory)
}

fn collect_bound_tree_entries(
    safety_root: &Path,
    directory: &Path,
    label: &str,
    inventory: &mut BTreeMap<String, BoundTreeEntry>,
) -> Result<()> {
    reject_symlink_components(safety_root, directory, "bound tree directory")?;
    let metadata = fs::symlink_metadata(directory).map_err(|source| {
        CodegenError::io("read bound tree directory metadata", directory, source)
    })?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "bound tree directory `{}` must be a real directory",
            directory.display()
        )));
    }

    let mut entries = fs::read_dir(directory)
        .map_err(|source| CodegenError::io("read bound tree directory", directory, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CodegenError::io("read bound tree entry", directory, source))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| CodegenError::io("read bound tree entry metadata", &path, source))?;
        if is_symlink_or_reparse_point(&metadata) {
            return Err(CodegenError::new(format!(
                "refusing bound tree symlink or reparse point `{}`",
                path.display()
            )));
        }

        let relative = normalized_relative_path(safety_root, &path)?;
        let kind = if metadata.is_dir() {
            BoundTreeEntryKind::Directory
        } else if metadata.is_file() {
            BoundTreeEntryKind::RegularFile
        } else {
            return Err(CodegenError::new(format!(
                "unsupported bound tree entry `{}`",
                path.display()
            )));
        };
        let expected = fs::canonicalize(&path).map_err(|source| {
            CodegenError::io(
                &format!("canonicalize {label} inventory entry"),
                &path,
                source,
            )
        })?;
        ensure_under(
            safety_root,
            &expected,
            &format!("approved {label} inventory entry"),
        )?;
        let identity = match kind {
            BoundTreeEntryKind::Directory => {
                let verified = open_verified_external_directory(&expected, label, |resolved| {
                    if resolved != expected {
                        return Err(CodegenError::new(format!(
                            "{label} inventory directory `{}` resolved as `{}`",
                            expected.display(),
                            resolved.display()
                        )));
                    }
                    ensure_under(
                        safety_root,
                        resolved,
                        &format!("approved {label} inventory directory"),
                    )
                })?;
                verified.identity_snapshot().map_err(|source| {
                    CodegenError::io(
                        &format!("identify {label} inventory directory"),
                        &expected,
                        source,
                    )
                })?
            }
            BoundTreeEntryKind::RegularFile => {
                let verified = open_verified_external_regular_file(&expected, label, |resolved| {
                    if resolved != expected {
                        return Err(CodegenError::new(format!(
                            "{label} inventory file `{}` resolved as `{}`",
                            expected.display(),
                            resolved.display()
                        )));
                    }
                    ensure_under(
                        safety_root,
                        resolved,
                        &format!("approved {label} inventory file"),
                    )
                })?;
                verified.identity_snapshot().map_err(|source| {
                    CodegenError::io(
                        &format!("identify {label} inventory file"),
                        &expected,
                        source,
                    )
                })?
            }
        };
        if inventory
            .insert(relative, BoundTreeEntry { kind, identity })
            .is_some()
        {
            return Err(CodegenError::new(format!(
                "duplicate normalized bound tree entry `{}`",
                path.display()
            )));
        }
        if kind == BoundTreeEntryKind::Directory {
            collect_bound_tree_entries(safety_root, &path, label, inventory)?;
        }
    }
    Ok(())
}

fn require_bound_tree_identity(
    observed: std::io::Result<VerifiedPathIdentity>,
    expected: VerifiedPathIdentity,
    label: &str,
    path: &Path,
) -> Result<()> {
    let observed = observed.map_err(|source| {
        CodegenError::io(&format!("identify opened {label} tree entry"), path, source)
    })?;
    if observed != expected {
        return Err(CodegenError::new(format!(
            "{label} tree entry `{}` changed identity after inventory",
            path.display()
        )));
    }
    Ok(())
}

pub fn read_tracked_bytes(path: &Path) -> Result<Vec<u8>> {
    let path = absolute_lexically_normalized(path, "tracked file")?;
    read_bytes(&path, ReadScope::Tracked)
}

pub fn read_external_bytes(path: &Path) -> Result<Vec<u8>> {
    let path = absolute_lexically_normalized(path, "external file")?;
    read_bytes(&path, ReadScope::External)
}

fn read_bytes(path: &Path, scope: ReadScope) -> Result<Vec<u8>> {
    let expected = fs::canonicalize(path)
        .map_err(|source| CodegenError::io("canonicalize expected file", path, source))?;
    let mut file = match scope {
        ReadScope::Tracked => {
            open_verified_tracked_regular_file(path, "tracked file", |resolved| {
                if resolved != expected {
                    return Err(CodegenError::new(format!(
                        "tracked file `{}` resolved to a different canonical file `{}`",
                        expected.display(),
                        resolved.display()
                    )));
                }
                Ok(())
            })?
        }
        ReadScope::External => {
            open_verified_external_regular_file(path, "external file", |resolved| {
                if resolved != expected {
                    return Err(CodegenError::new(format!(
                        "external file `{}` resolved to a different canonical file `{}`",
                        expected.display(),
                        resolved.display()
                    )));
                }
                Ok(())
            })?
        }
    };
    let mut bytes = Vec::new();
    file.file_mut()
        .read_to_end(&mut bytes)
        .map_err(|source| CodegenError::io("read file", path, source))?;
    Ok(bytes)
}

pub fn json_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    visit_files(root, root, &mut |path| {
        if is_json_file(path) {
            files.push(path.to_path_buf());
        }
        Ok(())
    })?;
    files.sort_by(|left, right| {
        normalized_relative_path(root, left)
            .expect("visited path is under root")
            .cmp(&normalized_relative_path(root, right).expect("visited path is under root"))
    });
    Ok(files)
}

pub fn read_tracked_tree(root: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let root = absolute_lexically_normalized(root, "tracked tree root")?;
    read_tree(&root, ReadScope::Tracked)
}

#[allow(dead_code, reason = "external counterpart of read_tracked_tree")]
pub fn read_external_tree(root: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let root = absolute_lexically_normalized(root, "external tree root")?;
    read_tree(&root, ReadScope::External)
}

fn read_tree(root: &Path, scope: ReadScope) -> Result<BTreeMap<String, Vec<u8>>> {
    if !root.exists() {
        return Ok(BTreeMap::new());
    }
    let root_metadata = fs::symlink_metadata(root)
        .map_err(|source| CodegenError::io("read tree root metadata", root, source))?;
    if is_symlink_or_reparse_point(&root_metadata) || !root_metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "read tree root `{}` must be a real directory",
            root.display()
        )));
    }
    let canonical_root = fs::canonicalize(root)
        .map_err(|source| CodegenError::io("canonicalize read tree root", root, source))?;
    let mut files = BTreeMap::new();
    visit_files(root, root, &mut |path| {
        let relative = normalized_relative_path(root, path)?;
        let expected = fs::canonicalize(path)
            .map_err(|source| CodegenError::io("canonicalize expected tree file", path, source))?;
        let mut verified = match scope {
            ReadScope::Tracked => {
                open_verified_tracked_regular_file(path, "tracked tree file", |resolved| {
                    if resolved != expected {
                        return Err(CodegenError::new(format!(
                            "tracked tree file `{}` resolved to a different canonical file `{}`",
                            expected.display(),
                            resolved.display()
                        )));
                    }
                    ensure_under(&canonical_root, resolved, "resolved tracked tree file")
                })?
            }
            ReadScope::External => {
                open_verified_external_regular_file(path, "external tree file", |resolved| {
                    if resolved != expected {
                        return Err(CodegenError::new(format!(
                            "external tree file `{}` resolved to a different canonical file `{}`",
                            expected.display(),
                            resolved.display()
                        )));
                    }
                    ensure_under(&canonical_root, resolved, "resolved external tree file")
                })?
            }
        };
        let mut bytes = Vec::new();
        verified
            .file_mut()
            .read_to_end(&mut bytes)
            .map_err(|source| CodegenError::io("read tree file", path, source))?;
        files.insert(relative, bytes);
        Ok(())
    })?;
    Ok(files)
}

fn absolute_lexically_normalized(path: &Path, label: &str) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(CodegenError::new(format!("{label} path must not be empty")));
    }
    // Resolve a relative input against exactly one current-directory sample.
    // Every subsequent validation and open uses the returned absolute path.
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        #[cfg(test)]
        CWD_CAPTURE_COUNT.with(|count| count.set(count.get() + 1));
        let captured_current_dir = std::env::current_dir()
            .map_err(|source| CodegenError::io(&format!("resolve {label}"), path, source))?;
        captured_current_dir.join(path)
    };

    let mut normalized = PathBuf::new();
    let mut normal_component_count = 0usize;
    for component in joined.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if normal_component_count == 0 {
                    return Err(CodegenError::new(format!(
                        "{label} `{}` escapes the filesystem root during lexical normalization",
                        path.display()
                    )));
                }
                let popped = normalized.pop();
                debug_assert!(popped);
                normal_component_count -= 1;
            }
            Component::Normal(segment) => {
                normalized.push(segment);
                normal_component_count += 1;
            }
        }
    }
    if !normalized.is_absolute() {
        return Err(CodegenError::new(format!(
            "{label} `{}` did not resolve to an absolute path",
            path.display()
        )));
    }
    Ok(normalized)
}

fn visit_files(
    safety_root: &Path,
    directory: &Path,
    visitor: &mut impl FnMut(&Path) -> Result<()>,
) -> Result<()> {
    reject_symlink_components(safety_root, directory, "walked directory")?;
    let metadata = fs::symlink_metadata(directory)
        .map_err(|source| CodegenError::io("read directory metadata", directory, source))?;
    if is_symlink_or_reparse_point(&metadata) {
        return Err(CodegenError::new(format!(
            "refusing to walk symlink or reparse point `{}`",
            directory.display()
        )));
    }
    if !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "walk root `{}` is not a directory",
            directory.display()
        )));
    }

    let mut entries = fs::read_dir(directory)
        .map_err(|source| CodegenError::io("read directory", directory, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CodegenError::io("read directory entry", directory, source))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| CodegenError::io("read file type", &path, source))?;
        if is_symlink_or_reparse_point(&metadata) {
            return Err(CodegenError::new(format!(
                "refusing to traverse symlink or reparse point `{}`",
                path.display()
            )));
        }
        if metadata.is_dir() {
            visit_files(safety_root, &path, visitor)?;
        } else if metadata.is_file() {
            visitor(&path)?;
        } else {
            return Err(CodegenError::new(format!(
                "unsupported filesystem entry `{}`",
                path.display()
            )));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomicTreeWrite {
    /// The exact previous output tree, preserved instead of recursively
    /// deleting a path that another process could substitute.
    pub preserved_previous: Option<PathBuf>,
}

pub fn write_tree_atomically(
    target: &Path,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<AtomicTreeWrite> {
    let parent = target.parent().ok_or_else(|| {
        CodegenError::new(format!(
            "generated output `{}` has no parent directory",
            target.display()
        ))
    })?;
    fs::create_dir_all(parent)
        .map_err(|source| CodegenError::io("create generated-output parent", parent, source))?;
    let parent_identity = Handle::from_path(parent)
        .map_err(|source| CodegenError::io("identify generated-output parent", parent, source))?;

    let previous_identity = if target.exists() {
        let metadata = fs::symlink_metadata(target)
            .map_err(|source| CodegenError::io("read generated-output metadata", target, source))?;
        if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
            return Err(CodegenError::new(format!(
                "generated output `{}` must be a real directory",
                target.display()
            )));
        }
        Some(Handle::from_path(target).map_err(|source| {
            CodegenError::io("identify previous generated output", target, source)
        })?)
    } else {
        None
    };

    let staging = unique_sibling(target, "staging")?;
    let backup = unique_sibling(target, "backup")?;
    fs::create_dir(&staging).map_err(|source| {
        CodegenError::io(
            "create generated-output staging directory",
            &staging,
            source,
        )
    })?;
    let staging_identity = Handle::from_path(&staging).map_err(|source| {
        CodegenError::io(
            "identify generated-output staging directory",
            &staging,
            source,
        )
    })?;

    populate_tree(&staging, files).map_err(|error| {
        CodegenError::new(format!(
            "{error}; incomplete staging tree was preserved at `{}` and must be reviewed before manual removal",
            staging.display()
        ))
    })?;
    require_directory_identity(
        parent,
        &parent_identity,
        "generated-output parent after staging",
    )?;
    require_directory_identity(&staging, &staging_identity, "generated-output staging tree")?;
    #[cfg(test)]
    BEFORE_TREE_INSTALL_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook(target);
        }
    });

    let Some(previous_identity) = previous_identity else {
        if target.exists() {
            return Err(CodegenError::new(format!(
                "generated output `{}` appeared after staging; refusing to replace it; staging was preserved at `{}`",
                target.display(),
                staging.display()
            )));
        }
        rename_no_replace(&staging, target).map_err(|source| {
            CodegenError::with_source(
                format!(
                    "failed to install fresh generated-output tree at `{}`; staging was preserved at `{}`",
                    target.display(),
                    staging.display()
                ),
                source,
            )
        })?;
        require_directory_identity(
            parent,
            &parent_identity,
            "generated-output parent after fresh install",
        )?;
        require_directory_identity(target, &staging_identity, "installed generated-output tree")?;
        return Ok(AtomicTreeWrite {
            preserved_previous: None,
        });
    };

    require_directory_identity(
        target,
        &previous_identity,
        "previous generated-output tree before replacement",
    )
    .map_err(|error| {
        CodegenError::new(format!(
            "{error}; staging was preserved at `{}`",
            staging.display()
        ))
    })?;
    rename_no_replace(target, &backup).map_err(|source| {
        CodegenError::with_source(
            format!(
                "failed to preserve previous generated-output tree from `{}` to `{}`; staging remains at `{}`",
                target.display(),
                backup.display(),
                staging.display()
            ),
            source,
        )
    })?;
    require_directory_identity(
        &backup,
        &previous_identity,
        "preserved previous generated-output tree",
    )?;
    if target.exists() {
        return Err(CodegenError::new(format!(
            "generated output `{}` was recreated during replacement; refusing to overwrite it; previous output is preserved at `{}` and staging at `{}`",
            target.display(),
            backup.display(),
            staging.display()
        )));
    }
    rename_no_replace(&staging, target).map_err(|source| {
        CodegenError::with_source(
            format!(
                "failed to install generated-output tree at `{}`; previous output is preserved at `{}` and staging at `{}`; no rollback or cleanup was attempted",
                target.display(),
                backup.display(),
                staging.display()
            ),
            source,
        )
    })?;
    require_directory_identity(
        parent,
        &parent_identity,
        "generated-output parent after replacement",
    )?;
    require_directory_identity(target, &staging_identity, "installed generated-output tree")?;
    require_directory_identity(
        &backup,
        &previous_identity,
        "preserved previous generated-output tree after install",
    )?;
    Ok(AtomicTreeWrite {
        preserved_previous: Some(backup),
    })
}

fn populate_tree(root: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    for (relative, bytes) in files {
        let components = crate::path::validate_portable_relative(relative, "generated file path")?;
        let mut path = root.to_path_buf();
        for component in components {
            path.push(component);
        }
        let parent = path.parent().expect("a generated file has a parent");
        fs::create_dir_all(parent)
            .map_err(|source| CodegenError::io("create generated directory", parent, source))?;
        write_new_file(&path, bytes)?;
    }
    sync_directory(root)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    let mut file = options
        .open(path)
        .map_err(|source| CodegenError::io("create generated file", path, source))?;
    file.write_all(bytes)
        .map_err(|source| CodegenError::io("write generated file", path, source))?;
    file.sync_all()
        .map_err(|source| CodegenError::io("sync generated file", path, source))
}

fn sync_directory(path: &Path) -> Result<()> {
    match File::open(path).and_then(|directory| directory.sync_all()) {
        Ok(()) => Ok(()),
        // Windows does not support opening a directory as a File. Rename is
        // still atomic within the volume; individual files were synced above.
        Err(source) if cfg!(windows) => {
            let _ = source;
            Ok(())
        }
        Err(source) => Err(CodegenError::io("sync generated directory", path, source)),
    }
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
fn rename_no_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, source, CWD, target, RenameFlags::NOREPLACE).map_err(Into::into)
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android", target_vendor = "apple"))
))]
fn rename_no_replace(_source: &Path, _target: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace directory publication is unsupported on this Unix target",
    ))
}

#[cfg(windows)]
fn rename_no_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    bir_rules_platform::rename_directory_no_replace(source, target)
}

#[cfg(not(any(unix, windows)))]
fn rename_no_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    let _ = (source, target);
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace directory publication is unsupported on this target",
    ))
}

fn unique_sibling(target: &Path, purpose: &str) -> Result<PathBuf> {
    let parent = target
        .parent()
        .ok_or_else(|| CodegenError::new(format!("path `{}` has no parent", target.display())))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            CodegenError::new(format!(
                "generated-output name `{}` is not valid UTF-8",
                target.display()
            ))
        })?;
    for _ in 0..1_000 {
        let sequence = UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            "{name}.bir-rules-codegen-{purpose}-{}-{sequence}",
            std::process::id()
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(CodegenError::new(format!(
        "could not allocate a unique {purpose} path beside `{}`",
        target.display()
    )))
}

fn require_directory_identity(path: &Path, expected: &Handle, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| CodegenError::io(&format!("inspect {label}"), path, source))?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "{label} `{}` is no longer a real directory",
            path.display()
        )));
    }
    let current = Handle::from_path(path)
        .map_err(|source| CodegenError::io(&format!("identify {label}"), path, source))?;
    if &current != expected {
        return Err(CodegenError::new(format!(
            "{label} `{}` was replaced during tree publication",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use super::{
        ApprovedExternalFile, ApprovedExternalRoot, BEFORE_TREE_INSTALL_HOOK, BOUND_TREE_READ_HOOK,
        CWD_CAPTURE_COUNT, read_external_bytes_bound, read_external_bytes_under,
        read_external_tree, read_external_tree_bound, read_external_tree_under, read_tracked_bytes,
        rename_no_replace, unique_sibling, write_tree_atomically,
    };

    fn temporary_directory(label: &str) -> std::path::PathBuf {
        let directory = crate::test_temp_dir().join(format!(
            "bir-rules-codegen-{label}-{}-{}",
            std::process::id(),
            super::UNIQUE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("create test directory");
        directory
    }

    fn relative_temporary_directory(label: &str) -> (PathBuf, PathBuf) {
        let relative = PathBuf::from("target").join(format!(
            "bir-rules-codegen-relative-{label}-{}-{}",
            std::process::id(),
            super::UNIQUE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let absolute = std::env::current_dir()
            .expect("current directory")
            .join(&relative);
        fs::create_dir_all(&absolute).expect("create relative test directory");
        (relative, absolute)
    }

    struct BoundTreeHookReset;

    impl Drop for BoundTreeHookReset {
        fn drop(&mut self) {
            BOUND_TREE_READ_HOOK.with(|slot| {
                slot.replace(None);
            });
        }
    }

    fn with_bound_tree_hook<T>(
        hook: impl Fn(&std::path::Path, &str, usize) + 'static,
        action: impl FnOnce() -> T,
    ) -> T {
        BOUND_TREE_READ_HOOK.with(|slot| {
            assert!(slot.replace(Some(Box::new(hook))).is_none());
        });
        let _reset = BoundTreeHookReset;
        action()
    }

    fn approve_external_tree(tree: &std::path::Path, label: &str) -> ApprovedExternalRoot {
        let canonical = fs::canonicalize(tree).expect("canonical approved tree");
        ApprovedExternalRoot::capture(&canonical, label, |resolved| {
            if resolved != canonical {
                return Err(crate::error::CodegenError::new(format!(
                    "{label} resolved outside its exact approved root"
                )));
            }
            Ok(())
        })
        .expect("capture approved external tree")
    }

    #[test]
    fn empty_file_and_root_paths_are_rejected_before_cwd_resolution() {
        let empty = std::path::Path::new("");
        let file_error = ApprovedExternalFile::capture(empty, "empty file", |_| Ok(()))
            .expect_err("empty external file path must fail");
        assert!(file_error.to_string().contains("must not be empty"));
        let root_error = ApprovedExternalRoot::capture(empty, "empty root", |_| Ok(()))
            .expect_err("empty external root path must fail");
        assert!(root_error.to_string().contains("must not be empty"));
        let tracked_error =
            read_tracked_bytes(empty).expect_err("empty tracked file path must fail");
        assert!(tracked_error.to_string().contains("must not be empty"));
    }

    #[test]
    fn bound_external_inputs_capture_relative_paths_once_and_normalize_them() {
        const CHILD_GUARD: &str = "BIR_RULES_RELATIVE_INPUT_CHILD";
        if std::env::var_os(CHILD_GUARD).is_none() {
            let child_cwd = temporary_directory("relative-input-child-cwd");
            let output = std::process::Command::new(
                std::env::current_exe().expect("resolve current test executable"),
            )
            .arg("--exact")
            .arg("files::tests::bound_external_inputs_capture_relative_paths_once_and_normalize_them")
            .arg("--nocapture")
            .env(CHILD_GUARD, "1")
            .current_dir(&child_cwd)
            .output()
            .expect("run relative-input child test");
            if !output.status.success() {
                panic!(
                    "relative-input child test failed\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            let relative_tracked = PathBuf::from(".")
                .join("unused")
                .join("..")
                .join("Cargo.toml");
            assert_eq!(
                read_tracked_bytes(&relative_tracked).expect("read relative tracked file"),
                fs::read("Cargo.toml").expect("read expected tracked file")
            );
            fs::remove_dir_all(child_cwd).expect("remove relative-input child cwd");
            return;
        }

        let (relative_root, absolute_root) = relative_temporary_directory("normalize");
        let file = absolute_root.join("input.bin");
        fs::write(&file, b"relative").expect("write relative fixture");
        let canonical_file = fs::canonicalize(&file).expect("canonical relative file");
        let canonical_root = fs::canonicalize(&absolute_root).expect("canonical relative root");
        let relative_file = relative_root
            .join("unused")
            .join("..")
            .join(".")
            .join("input.bin");

        CWD_CAPTURE_COUNT.with(|count| count.set(0));
        let approved_file =
            ApprovedExternalFile::capture(&relative_file, "relative file", |resolved| {
                assert_eq!(resolved, canonical_file);
                Ok(())
            })
            .expect("capture relative external file");
        assert_eq!(
            read_external_bytes_bound(approved_file, "relative file")
                .expect("read retained relative external file"),
            b"relative"
        );
        CWD_CAPTURE_COUNT.with(|count| {
            assert_eq!(
                count.get(),
                1,
                "bound read must not sample the current directory again"
            );
        });

        let approved_root =
            ApprovedExternalRoot::capture(&relative_root, "relative root", |resolved| {
                assert_eq!(resolved, canonical_root);
                Ok(())
            })
            .expect("capture relative external root");
        assert_eq!(
            read_external_tree_bound(&approved_root, "relative root")
                .expect("read retained relative root"),
            BTreeMap::from([("input.bin".to_owned(), b"relative".to_vec())])
        );

        drop(approved_root);
        fs::remove_dir_all(absolute_root).expect("remove relative fixture");
    }

    #[test]
    fn approved_external_file_retains_or_rejects_original_identity() {
        let root = temporary_directory("approved-file-identity");
        let path = root.join("source.bin");
        let displaced = root.join("source-displaced.bin");
        fs::write(&path, b"approved").expect("write approved file");
        let canonical = fs::canonicalize(&path).expect("canonical approved file");
        let approved = ApprovedExternalFile::capture(&canonical, "approved file", |resolved| {
            if resolved != canonical {
                return Err(crate::error::CodegenError::new(
                    "approved file resolved elsewhere",
                ));
            }
            Ok(())
        })
        .expect("capture approved file");

        match fs::rename(&path, &displaced) {
            Ok(()) => {
                fs::write(&path, b"replacement").expect("write replacement file");
                let error = read_external_bytes_bound(approved, "approved file")
                    .expect_err("same-path replacement must not be read");
                assert!(error.to_string().contains("approved identity"));
            }
            Err(_) => {
                assert_eq!(
                    read_external_bytes_bound(approved, "approved file")
                        .expect("retained restrictive file"),
                    b"approved"
                );
            }
        }
        fs::remove_dir_all(root).expect("remove approved file fixture");
    }

    #[test]
    fn approved_external_root_rejects_substitution_or_blocks_it() {
        let root = temporary_directory("approved-root-identity");
        let tree = root.join("tree");
        let displaced = root.join("tree-displaced");
        fs::create_dir(&tree).expect("create approved tree");
        fs::write(tree.join("approved.bin"), b"approved").expect("write approved tree file");
        let canonical = fs::canonicalize(&tree).expect("canonical approved tree");
        let approved = ApprovedExternalRoot::capture(&canonical, "approved tree", |resolved| {
            if resolved != canonical {
                return Err(crate::error::CodegenError::new(
                    "approved tree resolved elsewhere",
                ));
            }
            Ok(())
        })
        .expect("capture approved tree");

        match fs::rename(&tree, &displaced) {
            Ok(()) => {
                fs::create_dir(&tree).expect("create replacement tree");
                fs::write(tree.join("approved.bin"), b"replacement")
                    .expect("write replacement tree file");
                let error = read_external_tree_bound(&approved, "approved tree")
                    .expect_err("replacement root must fail");
                assert!(error.to_string().contains("approved identity"));
            }
            Err(_) => {
                assert_eq!(
                    read_external_tree_bound(&approved, "approved tree")
                        .expect("retained restrictive root"),
                    BTreeMap::from([("approved.bin".to_owned(), b"approved".to_vec())])
                );
            }
        }
        drop(approved);
        fs::remove_dir_all(root).expect("remove approved root fixture");
    }

    #[test]
    fn bound_tree_rejects_persistent_same_name_child_replacement_or_proves_it_locked() {
        let root = temporary_directory("bound-child-persistent");
        let tree = root.join("tree");
        fs::create_dir(&tree).expect("create tree");
        let child = tree.join("value.bin");
        let displaced = tree.join("value-original.bin");
        fs::write(&child, b"original").expect("write original child");
        let approved = approve_external_tree(&tree, "persistent child tree");
        let replaced = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let child_for_hook = child.clone();
        let displaced_for_hook = displaced.clone();
        let replaced_for_hook = replaced.clone();
        let result = with_bound_tree_hook(
            move |_, phase, _| {
                if phase == "after-all-open"
                    && fs::rename(&child_for_hook, &displaced_for_hook).is_ok()
                {
                    fs::write(&child_for_hook, b"attacker").expect("install replacement child");
                    replaced_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            || read_external_tree_bound(&approved, "persistent child tree"),
        );

        if replaced.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                result.is_err(),
                "a persistent same-name child replacement must return Err, never a map"
            );
            fs::remove_file(&child).expect("remove attacker child");
            fs::rename(&displaced, &child).expect("restore original child");
        } else {
            assert_eq!(
                result.expect("restrictive child handle blocked replacement"),
                BTreeMap::from([("value.bin".to_owned(), b"original".to_vec())])
            );
        }
        drop(approved);
        fs::remove_dir_all(root).expect("remove persistent child fixture");
    }

    #[test]
    fn bound_tree_baseline_rejects_not_yet_open_child_aba() {
        let root = temporary_directory("bound-child-aba");
        let tree = root.join("tree");
        fs::create_dir(&tree).expect("create tree");
        fs::write(tree.join("a.bin"), b"first").expect("write first child");
        let child = tree.join("b.bin");
        let displaced = tree.join("b-original.bin");
        let attacker_saved = tree.join("b-attacker.bin");
        fs::write(&child, b"second").expect("write second child");
        let approved = approve_external_tree(&tree, "child ABA tree");
        let replaced = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let restored = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let child_for_hook = child.clone();
        let displaced_for_hook = displaced.clone();
        let attacker_saved_for_hook = attacker_saved.clone();
        let replaced_for_hook = replaced.clone();
        let restored_for_hook = restored.clone();
        let result = with_bound_tree_hook(
            move |_, phase, index| {
                if phase == "after-open"
                    && index == 0
                    && fs::rename(&child_for_hook, &displaced_for_hook).is_ok()
                {
                    fs::write(&child_for_hook, b"attack").expect("install ABA child");
                    replaced_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                } else if phase == "after-entry-open"
                    && index == 1
                    && replaced_for_hook.load(std::sync::atomic::Ordering::SeqCst)
                    && fs::rename(&child_for_hook, &attacker_saved_for_hook).is_ok()
                {
                    fs::rename(&displaced_for_hook, &child_for_hook)
                        .expect("restore original child name");
                    restored_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            || read_external_tree_bound(&approved, "child ABA tree"),
        );

        if replaced.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                result.is_err(),
                "the baseline child identity must reject ABA and return no map"
            );
            if restored.load(std::sync::atomic::Ordering::SeqCst) {
                fs::remove_file(&attacker_saved).expect("remove saved attacker child");
            } else {
                fs::remove_file(&child).expect("remove current attacker child");
                fs::rename(&displaced, &child).expect("restore original child after locked ABA");
            }
        } else {
            assert_eq!(
                result.expect("retained ancestor handle blocked not-yet-open child replacement"),
                BTreeMap::from([
                    ("a.bin".to_owned(), b"first".to_vec()),
                    ("b.bin".to_owned(), b"second".to_vec()),
                ])
            );
        }
        drop(approved);
        fs::remove_dir_all(root).expect("remove child ABA fixture");
    }

    #[test]
    fn bound_tree_rejects_root_aba_or_proves_root_share_lock() {
        let root = temporary_directory("bound-root-aba");
        let tree = root.join("tree");
        let displaced = root.join("tree-original");
        let attacker_saved = root.join("tree-attacker");
        fs::create_dir(&tree).expect("create tree");
        fs::write(tree.join("a.bin"), b"first").expect("write first child");
        fs::write(tree.join("b.bin"), b"second").expect("write second child");
        let approved = approve_external_tree(&tree, "root ABA tree");
        let swapped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let restored = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let tree_for_hook = tree.clone();
        let displaced_for_hook = displaced.clone();
        let attacker_saved_for_hook = attacker_saved.clone();
        let swapped_for_hook = swapped.clone();
        let restored_for_hook = restored.clone();
        let result = with_bound_tree_hook(
            move |_, phase, index| {
                if phase == "after-open"
                    && index == 0
                    && fs::rename(&tree_for_hook, &displaced_for_hook).is_ok()
                {
                    fs::create_dir(&tree_for_hook).expect("create attacker root");
                    fs::write(tree_for_hook.join("a.bin"), b"attack-a")
                        .expect("write attacker first child");
                    fs::write(tree_for_hook.join("b.bin"), b"attack-b")
                        .expect("write attacker second child");
                    swapped_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                } else if phase == "after-entry-open"
                    && index == 1
                    && swapped_for_hook.load(std::sync::atomic::Ordering::SeqCst)
                    && fs::rename(&tree_for_hook, &attacker_saved_for_hook).is_ok()
                {
                    fs::rename(&displaced_for_hook, &tree_for_hook).expect("restore original root");
                    restored_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            || read_external_tree_bound(&approved, "root ABA tree"),
        );

        if swapped.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                result.is_err(),
                "a root ABA/mixed generation must return Err, never a map"
            );
            if restored.load(std::sync::atomic::Ordering::SeqCst) {
                fs::remove_dir_all(&attacker_saved).expect("remove saved attacker root");
            } else {
                fs::remove_dir_all(&tree).expect("remove current attacker root");
                fs::rename(&displaced, &tree).expect("restore original root after ABA");
            }
        } else {
            assert_eq!(
                result.expect("retained root handle blocked substitution"),
                BTreeMap::from([
                    ("a.bin".to_owned(), b"first".to_vec()),
                    ("b.bin".to_owned(), b"second".to_vec()),
                ])
            );
        }
        drop(approved);
        fs::remove_dir_all(root).expect("remove root ABA fixture");
    }

    #[test]
    fn bound_tree_rejects_nested_directory_aba() {
        let root = temporary_directory("bound-nested-aba");
        let tree = root.join("tree");
        let nested = tree.join("nested");
        let displaced = tree.join("nested-original");
        let attacker_saved = tree.join("nested-attacker");
        fs::create_dir_all(&nested).expect("create nested tree");
        fs::write(tree.join("a.bin"), b"first").expect("write first child");
        fs::write(nested.join("value.bin"), b"nested").expect("write nested child");
        let approved = approve_external_tree(&tree, "nested ABA tree");
        let swapped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let restored = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let nested_for_hook = nested.clone();
        let displaced_for_hook = displaced.clone();
        let attacker_saved_for_hook = attacker_saved.clone();
        let swapped_for_hook = swapped.clone();
        let restored_for_hook = restored.clone();
        let result = with_bound_tree_hook(
            move |_, phase, index| {
                if phase == "after-open"
                    && index == 0
                    && fs::rename(&nested_for_hook, &displaced_for_hook).is_ok()
                {
                    fs::create_dir(&nested_for_hook).expect("create attacker nested directory");
                    fs::write(nested_for_hook.join("value.bin"), b"attack")
                        .expect("write attacker nested child");
                    swapped_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                } else if phase == "after-entry-open"
                    && index == 1
                    && swapped_for_hook.load(std::sync::atomic::Ordering::SeqCst)
                    && fs::rename(&nested_for_hook, &attacker_saved_for_hook).is_ok()
                {
                    fs::rename(&displaced_for_hook, &nested_for_hook)
                        .expect("restore original nested directory");
                    restored_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            || read_external_tree_bound(&approved, "nested ABA tree"),
        );

        if swapped.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                result.is_err(),
                "nested directory ABA must return Err, never a map"
            );
            if restored.load(std::sync::atomic::Ordering::SeqCst) {
                fs::remove_dir_all(&attacker_saved).expect("remove attacker nested directory");
            } else {
                fs::remove_dir_all(&nested).expect("remove current attacker nested directory");
                fs::rename(&displaced, &nested).expect("restore original nested directory");
            }
        } else {
            assert_eq!(
                result.expect("retained ancestor handle blocked nested replacement"),
                BTreeMap::from([
                    ("a.bin".to_owned(), b"first".to_vec()),
                    ("nested/value.bin".to_owned(), b"nested".to_vec(),),
                ])
            );
        }
        drop(approved);
        fs::remove_dir_all(root).expect("remove nested ABA fixture");
    }

    #[test]
    fn bound_tree_rejects_entry_added_after_inventory() {
        let root = temporary_directory("bound-add-entry");
        let tree = root.join("tree");
        fs::create_dir(&tree).expect("create tree");
        fs::write(tree.join("value.bin"), b"original").expect("write original");
        let added = tree.join("added.bin");
        let approved = approve_external_tree(&tree, "add-entry tree");
        let added_for_hook = added.clone();
        let result = with_bound_tree_hook(
            move |_, phase, _| {
                if phase == "after-all-open" {
                    fs::write(&added_for_hook, b"late").expect("add late inventory entry");
                }
            },
            || read_external_tree_bound(&approved, "add-entry tree"),
        );
        assert!(
            result.is_err(),
            "a late added entry must return Err, never a partial map"
        );
        fs::remove_file(added).expect("remove added entry");
        drop(approved);
        fs::remove_dir_all(root).expect("remove add-entry fixture");
    }

    #[test]
    fn bound_tree_rejects_removed_entry_or_proves_file_share_lock() {
        let root = temporary_directory("bound-remove-entry");
        let tree = root.join("tree");
        fs::create_dir(&tree).expect("create tree");
        let child = tree.join("value.bin");
        fs::write(&child, b"original").expect("write original");
        let approved = approve_external_tree(&tree, "remove-entry tree");
        let removed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let child_for_hook = child.clone();
        let removed_for_hook = removed.clone();
        let result = with_bound_tree_hook(
            move |_, phase, _| {
                if phase == "after-all-open" && fs::remove_file(&child_for_hook).is_ok() {
                    removed_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            || read_external_tree_bound(&approved, "remove-entry tree"),
        );
        if removed.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                result.is_err(),
                "a removed entry must return Err, never a map"
            );
            fs::write(&child, b"original").expect("restore removed child");
        } else {
            assert_eq!(
                result.expect("retained file handle blocked removal"),
                BTreeMap::from([("value.bin".to_owned(), b"original".to_vec())])
            );
        }
        drop(approved);
        fs::remove_dir_all(root).expect("remove remove-entry fixture");
    }

    #[test]
    fn bound_tree_rejects_directory_to_file_type_swap_or_proves_directory_lock() {
        let root = temporary_directory("bound-type-swap");
        let tree = root.join("tree");
        let nested = tree.join("nested");
        let displaced = tree.join("nested-original");
        fs::create_dir_all(&nested).expect("create nested tree");
        fs::write(nested.join("value.bin"), b"original").expect("write nested child");
        let approved = approve_external_tree(&tree, "type-swap tree");
        let swapped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let nested_for_hook = nested.clone();
        let displaced_for_hook = displaced.clone();
        let swapped_for_hook = swapped.clone();
        let result = with_bound_tree_hook(
            move |_, phase, _| {
                if phase == "after-all-open"
                    && fs::rename(&nested_for_hook, &displaced_for_hook).is_ok()
                {
                    fs::write(&nested_for_hook, b"now-a-file").expect("install type-swapped entry");
                    swapped_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            || read_external_tree_bound(&approved, "type-swap tree"),
        );
        if swapped.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                result.is_err(),
                "a directory-to-file swap must return Err, never a map"
            );
            fs::remove_file(&nested).expect("remove type-swapped file");
            fs::rename(&displaced, &nested).expect("restore nested directory");
        } else {
            assert_eq!(
                result.expect("retained nested handles blocked type swap"),
                BTreeMap::from([("nested/value.bin".to_owned(), b"original".to_vec())])
            );
        }
        drop(approved);
        fs::remove_dir_all(root).expect("remove type-swap fixture");
    }

    #[test]
    fn bound_tree_rejects_hard_link_introduced_after_capture_when_supported() {
        let root = temporary_directory("bound-hardlink-injection");
        let tree = root.join("tree");
        fs::create_dir(&tree).expect("create tree");
        let child = tree.join("value.bin");
        let alias = tree.join("alias.bin");
        fs::write(&child, b"original").expect("write original");
        let approved = approve_external_tree(&tree, "hardlink-injection tree");
        let linked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let child_for_hook = child.clone();
        let alias_for_hook = alias.clone();
        let linked_for_hook = linked.clone();
        let result = with_bound_tree_hook(
            move |_, phase, _| {
                if phase == "after-all-open"
                    && fs::hard_link(&child_for_hook, &alias_for_hook).is_ok()
                {
                    linked_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            || read_external_tree_bound(&approved, "hardlink-injection tree"),
        );
        if linked.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                result.is_err(),
                "a late hard-link alias must return Err, never a map"
            );
            fs::remove_file(&alias).expect("remove hard-link alias");
        } else {
            assert_eq!(
                result.expect("hard-link creation was blocked or unsupported"),
                BTreeMap::from([("value.bin".to_owned(), b"original".to_vec())])
            );
        }
        drop(approved);
        fs::remove_dir_all(root).expect("remove hardlink-injection fixture");
    }

    #[test]
    fn bound_tree_rejects_symlink_or_reparse_injected_after_capture_when_supported() {
        let root = temporary_directory("bound-symlink-injection");
        let tree = root.join("tree");
        fs::create_dir(&tree).expect("create tree");
        let child = tree.join("value.bin");
        let alias = tree.join("alias.bin");
        fs::write(&child, b"original").expect("write original");
        let approved = approve_external_tree(&tree, "symlink-injection tree");
        let linked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let child_for_hook = child.clone();
        let alias_for_hook = alias.clone();
        let linked_for_hook = linked.clone();
        let result = with_bound_tree_hook(
            move |_, phase, _| {
                if phase != "after-all-open" {
                    return;
                }
                #[cfg(unix)]
                let created = std::os::unix::fs::symlink(&child_for_hook, &alias_for_hook).is_ok();
                #[cfg(windows)]
                let created =
                    std::os::windows::fs::symlink_file(&child_for_hook, &alias_for_hook).is_ok();
                if created {
                    linked_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            || read_external_tree_bound(&approved, "symlink-injection tree"),
        );
        if linked.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                result.is_err(),
                "a late symlink/reparse entry must return Err, never a map"
            );
            fs::remove_file(&alias).expect("remove symlink alias");
        } else {
            assert_eq!(
                result.expect("symlink creation was unavailable"),
                BTreeMap::from([("value.bin".to_owned(), b"original".to_vec())])
            );
        }
        drop(approved);
        fs::remove_dir_all(root).expect("remove symlink-injection fixture");
    }

    #[test]
    fn bound_tree_rejects_same_size_in_place_mutation_or_proves_write_share_lock() {
        let root = temporary_directory("bound-in-place-mutation");
        let tree = root.join("tree");
        fs::create_dir(&tree).expect("create tree");
        let child = tree.join("value.bin");
        fs::write(&child, b"original").expect("write original");
        let approved = approve_external_tree(&tree, "in-place mutation tree");
        let mutated = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let child_for_hook = child.clone();
        let mutated_for_hook = mutated.clone();
        let result = with_bound_tree_hook(
            move |_, phase, index| {
                if phase == "after-first-read"
                    && index == 0
                    && fs::write(&child_for_hook, b"mutated!").is_ok()
                {
                    mutated_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            || read_external_tree_bound(&approved, "in-place mutation tree"),
        );
        if mutated.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                result.is_err(),
                "same-size in-place mutation must return Err, never a map"
            );
        } else {
            assert_eq!(
                result.expect("retained file handle blocked in-place writer"),
                BTreeMap::from([("value.bin".to_owned(), b"original".to_vec())])
            );
        }
        drop(approved);
        fs::remove_dir_all(root).expect("remove in-place mutation fixture");
    }

    #[test]
    fn late_failure_after_prior_file_read_returns_no_partial_map() {
        let root = temporary_directory("bound-late-failure");
        let tree = root.join("tree");
        fs::create_dir(&tree).expect("create tree");
        fs::write(tree.join("a.bin"), b"first").expect("write first child");
        fs::write(tree.join("b.bin"), b"second").expect("write second child");
        let added = tree.join("late-added.bin");
        let approved = approve_external_tree(&tree, "late-failure tree");
        let added_for_hook = added.clone();
        let result = with_bound_tree_hook(
            move |_, phase, index| {
                if phase == "after-first-read" && index == 1 {
                    fs::write(&added_for_hook, b"late").expect("add entry after first map insert");
                }
            },
            || read_external_tree_bound(&approved, "late-failure tree"),
        );
        assert!(
            result.is_err(),
            "a final inventory failure must expose Err, not the locally accumulated map"
        );
        fs::remove_file(added).expect("remove late-added entry");
        drop(approved);
        fs::remove_dir_all(root).expect("remove late-failure fixture");
    }

    #[test]
    fn bound_child_file_read_revalidates_parent_root_after_substitution_attempt() {
        let root = temporary_directory("bound-file-under-root");
        let tree = root.join("tree");
        let displaced = root.join("tree-original");
        fs::create_dir(&tree).expect("create tree");
        let child = tree.join("value.bin");
        fs::write(&child, b"original").expect("write original");
        let approved = approve_external_tree(&tree, "file-under-root");
        let swapped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let tree_for_hook = tree.clone();
        let displaced_for_hook = displaced.clone();
        let swapped_for_hook = swapped.clone();
        let result = with_bound_tree_hook(
            move |_, phase, _| {
                if phase == "after-first-read"
                    && fs::rename(&tree_for_hook, &displaced_for_hook).is_ok()
                {
                    fs::create_dir(&tree_for_hook).expect("create replacement parent root");
                    fs::write(tree_for_hook.join("value.bin"), b"attacker")
                        .expect("write replacement child");
                    swapped_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            || read_external_bytes_under(&approved, &child, "file-under-root"),
        );
        if swapped.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                result.is_err(),
                "parent substitution after a retained child read must return Err"
            );
            fs::remove_dir_all(&tree).expect("remove replacement root");
            fs::rename(&displaced, &tree).expect("restore original root");
        } else {
            assert_eq!(
                result.expect("retained parent root blocked substitution"),
                b"original"
            );
        }
        drop(approved);
        fs::remove_dir_all(root).expect("remove file-under-root fixture");
    }

    #[test]
    fn bound_subtree_read_rejects_subtree_substitution_or_proves_share_lock() {
        let root = temporary_directory("bound-subtree-under-root");
        let parent = root.join("parent");
        let child = parent.join("child");
        let displaced = parent.join("child-original");
        fs::create_dir_all(&child).expect("create subtree");
        fs::write(child.join("value.bin"), b"original").expect("write subtree child");
        let approved_parent = approve_external_tree(&parent, "subtree parent");
        let swapped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let child_for_hook = child.clone();
        let displaced_for_hook = displaced.clone();
        let swapped_for_hook = swapped.clone();
        let result = with_bound_tree_hook(
            move |_, phase, index| {
                if phase == "after-open"
                    && index == 0
                    && fs::rename(&child_for_hook, &displaced_for_hook).is_ok()
                {
                    fs::create_dir(&child_for_hook).expect("create replacement subtree");
                    fs::write(child_for_hook.join("value.bin"), b"attacker")
                        .expect("write replacement subtree child");
                    swapped_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            || read_external_tree_under(&approved_parent, &child, "subtree child"),
        );
        if swapped.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                result.is_err(),
                "subtree substitution must return Err, never a map"
            );
            fs::remove_dir_all(&child).expect("remove replacement subtree");
            fs::rename(&displaced, &child).expect("restore original subtree");
        } else {
            assert_eq!(
                result.expect("retained subtree root blocked substitution"),
                BTreeMap::from([("value.bin".to_owned(), b"original".to_vec())])
            );
        }
        drop(approved_parent);
        fs::remove_dir_all(root).expect("remove subtree fixture");
    }

    #[cfg(windows)]
    #[test]
    fn strict_external_local_root_handle_blocks_rename_until_drop() {
        let root = temporary_directory("external-root-share-lock");
        let tree = root.join("tree");
        let renamed = root.join("renamed-tree");
        fs::create_dir(&tree).expect("create local tree");
        let approved = approve_external_tree(&tree, "local root share lock");
        assert!(
            fs::rename(&tree, &renamed).is_err(),
            "retained external directory handle must deny rename sharing"
        );
        drop(approved);
        fs::rename(&tree, &renamed).expect("rename control succeeds after handle drop");
        fs::remove_dir_all(root).expect("remove local root share-lock fixture");
    }

    #[cfg(windows)]
    #[test]
    fn strict_external_scope_rejects_actual_unc_provider_error_87() {
        let manifest_dir =
            fs::canonicalize(env!("CARGO_MANIFEST_DIR")).expect("canonical manifest directory");
        let text = manifest_dir.to_string_lossy();
        if !text.starts_with(r"\\?\UNC\") && !text.starts_with(r"\\") {
            eprintln!("skipping actual UNC E87 check from a local checkout");
            return;
        }
        let manifest = manifest_dir.join("Cargo.toml");
        let file_error = ApprovedExternalFile::capture(&manifest, "strict UNC file", |_| Ok(()))
            .expect_err("this observed UNC provider must reject strict FILE_ID_INFO");
        let file_debug = format!("{file_error:?}");
        assert!(
            file_debug.contains("code: 87") || file_debug.contains("os error 87"),
            "strict UNC file rejection must retain ERROR_INVALID_PARAMETER: {file_error:?}"
        );
        let root_error =
            ApprovedExternalRoot::capture(&manifest_dir, "strict UNC root", |_| Ok(()))
                .expect_err("this observed UNC provider must reject strict directory identity");
        let root_debug = format!("{root_error:?}");
        assert!(
            root_debug.contains("code: 87") || root_debug.contains("os error 87"),
            "strict UNC root rejection must retain ERROR_INVALID_PARAMETER: {root_error:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn drive_relative_external_paths_are_not_treated_as_absolute() {
        let error = ApprovedExternalFile::capture(
            std::path::Path::new("C:drive-relative.bin"),
            "drive-relative file",
            |_| Ok(()),
        )
        .expect_err("drive-relative path must not gain absolute semantics");
        assert!(
            error
                .to_string()
                .contains("did not resolve to an absolute path"),
            "unexpected drive-relative error: {error}"
        );
    }

    #[test]
    fn atomic_tree_replacement_removes_stale_files() {
        let root = temporary_directory("replace");
        let target = root.join("generated");
        let first = BTreeMap::from([
            ("old.rs".to_owned(), b"old".to_vec()),
            ("nested/keep.rs".to_owned(), b"before".to_vec()),
        ]);
        let first_write = write_tree_atomically(&target, &first).expect("first write");
        assert_eq!(first_write.preserved_previous, None);

        let second = BTreeMap::from([("nested/keep.rs".to_owned(), b"after".to_vec())]);
        let second_write = write_tree_atomically(&target, &second).expect("replacement");

        assert_eq!(read_external_tree(&target).expect("read output"), second);
        let preserved = second_write
            .preserved_previous
            .expect("replacement preserves the previous output");
        assert_eq!(
            read_external_tree(&preserved).expect("read preserved output"),
            first
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn temporary_siblings_are_not_dot_prefixed() {
        let root = temporary_directory("temporary-name");
        let target = root.join("generated");
        let sibling = unique_sibling(&target, "backup").expect("allocate sibling");
        let name = sibling
            .file_name()
            .and_then(|name| name.to_str())
            .expect("temporary name is UTF-8");

        // Samba exposes dot-prefixed directories through Windows as HIDDEN.
        // Rust's recursive removal can then fail with ERROR_INVALID_PARAMETER
        // for an extended-length UNC path after the atomic swap succeeds.
        assert!(!name.starts_with('.'));
        assert!(name.contains(".bir-rules-codegen-backup-"));

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn directory_publication_never_replaces_an_existing_target() {
        let root = temporary_directory("rename-no-replace");
        let source = root.join("source");
        let target = root.join("target");
        fs::create_dir(&source).expect("create source");
        fs::create_dir(&target).expect("create target");
        fs::write(source.join("source.txt"), b"source").expect("write source");

        rename_no_replace(&source, &target).expect_err("existing target must block rename");
        assert_eq!(
            fs::read(source.join("source.txt")).expect("source survives"),
            b"source"
        );
        assert!(target.is_dir(), "empty destination directory survives");
        assert_eq!(
            fs::read_dir(&target)
                .expect("read empty destination")
                .count(),
            0
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn replacement_refuses_and_preserves_a_substituted_unrelated_directory() {
        let root = temporary_directory("substituted-target");
        let target = root.join("generated");
        let displaced = root.join("displaced-original");
        let unrelated = root.join("unrelated");
        let first = BTreeMap::from([("old.rs".to_owned(), b"old".to_vec())]);
        write_tree_atomically(&target, &first).expect("first write");
        fs::create_dir(&unrelated).expect("create unrelated directory");
        fs::write(unrelated.join("sentinel.txt"), b"must survive").expect("write sentinel");

        let target_for_hook = target.clone();
        let displaced_for_hook = displaced.clone();
        let unrelated_for_hook = unrelated.clone();
        BEFORE_TREE_INSTALL_HOOK.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move |observed_target| {
                assert_eq!(observed_target, target_for_hook);
                fs::rename(&target_for_hook, &displaced_for_hook)
                    .expect("displace validated output");
                fs::rename(&unrelated_for_hook, &target_for_hook)
                    .expect("substitute unrelated directory");
            }));
        });

        let second = BTreeMap::from([("new.rs".to_owned(), b"new".to_vec())]);
        let error = write_tree_atomically(&target, &second)
            .expect_err("substituted target must fail closed");
        assert!(error.to_string().contains("was replaced"));
        assert_eq!(
            fs::read(target.join("sentinel.txt")).expect("read surviving sentinel"),
            b"must survive"
        );
        assert_eq!(
            read_external_tree(&displaced).expect("read displaced original"),
            first
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn read_tree_rejects_hard_link_aliases_before_reading_bytes() {
        let root = temporary_directory("hard-link");
        let source = root.join("source.json");
        let alias = root.join("alias.json");
        fs::write(&source, b"official").expect("write source");
        fs::hard_link(&source, &alias).expect("create hard link");

        let error = read_external_tree(&root).expect_err("hard-linked tree entry must fail closed");
        assert!(error.to_string().contains("hard links"));
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
