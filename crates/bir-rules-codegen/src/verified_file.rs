//! Race-resistant reads of external regular files.
//!
//! Callers validate the resolved path both before and after the file is
//! opened. The opened handle is then compared with a fresh handle for the
//! validated path before any caller is allowed to read bytes. This prevents a
//! path replacement from redirecting a read after the policy checks.

use std::fs::{self, File, Metadata};
use std::path::{Path, PathBuf};

use same_file::Handle;

use crate::error::{CodegenError, Result};

#[cfg(windows)]
const WINDOWS_LINK_COUNT_ATTEMPTS: usize = 32;

#[derive(Debug)]
pub(crate) struct VerifiedRegularFile {
    canonical_path: PathBuf,
    handle: Handle,
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
}

/// Open an external regular file without exposing its bytes until the opened
/// handle is proven to be the same file as a freshly revalidated path.
pub(crate) fn open_verified_regular_file<F>(
    path: &Path,
    label: &str,
    validate_resolved_path: F,
) -> Result<VerifiedRegularFile>
where
    F: Fn(&Path) -> Result<()>,
{
    let canonical_before = canonical_real_regular_file(path, label)?;
    validate_resolved_path(&canonical_before)?;

    // Opening can race with a same-user path replacement. Do not read from the
    // handle until the post-open path and file identity checks below pass.
    let file = File::open(&canonical_before)
        .map_err(|source| CodegenError::io(&format!("open {label}"), &canonical_before, source))?;
    let opened_metadata = file.metadata().map_err(|source| {
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
    let opened_handle = Handle::from_file(file).map_err(|source| {
        CodegenError::io(
            &format!("identify opened {label}"),
            &canonical_before,
            source,
        )
    })?;

    let canonical_after = canonical_real_regular_file(&canonical_before, label)?;
    validate_resolved_path(&canonical_after)?;
    if !same_canonical_path(&canonical_before, &canonical_after) {
        return Err(CodegenError::new(format!(
            "{label} path changed while it was being opened (before `{}`, after `{}`)",
            canonical_before.display(),
            canonical_after.display()
        )));
    }

    let current_handle = Handle::from_path(&canonical_after).map_err(|source| {
        CodegenError::io(
            &format!("identify post-open {label}"),
            &canonical_after,
            source,
        )
    })?;
    if opened_handle != current_handle {
        return Err(CodegenError::new(format!(
            "{label} `{}` was replaced while it was being opened; refusing to read",
            canonical_after.display()
        )));
    }

    // A final resolution check closes the window between the second
    // canonicalization and the identity lookup. Once it passes, later path
    // replacement cannot redirect the already-opened handle.
    let canonical_final = canonical_real_regular_file(&canonical_after, label)?;
    validate_resolved_path(&canonical_final)?;
    if !same_canonical_path(&canonical_before, &canonical_final) {
        return Err(CodegenError::new(format!(
            "{label} path changed during identity verification (before `{}`, final `{}`)",
            canonical_before.display(),
            canonical_final.display()
        )));
    }
    let final_handle = Handle::from_path(&canonical_final).map_err(|source| {
        CodegenError::io(&format!("identify final {label}"), &canonical_final, source)
    })?;
    if opened_handle != final_handle {
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
        opened_handle.as_file(),
        &final_metadata,
        &canonical_final,
        label,
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
    _file: &File,
    metadata: &Metadata,
    path: &Path,
    label: &str,
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
    file: &File,
    _metadata: &Metadata,
    path: &Path,
    label: &str,
) -> Result<()> {
    let link_count = stable_windows_link_count(file, path, label)?;
    if link_count != 1 {
        return Err(CodegenError::new(format!(
            "{label} `{}` has {link_count} hard links; aliased external inputs are forbidden",
            path.display()
        )));
    }
    Ok(())
}

/// Obtain a trustworthy Windows hard-link count from one already-opened file.
///
/// Some shared-filesystem redirectors transiently return the invalid value
/// zero for `nNumberOfLinks` under load. Zero is never accepted as safe: retry
/// the same handle with a one-millisecond bounded backoff, then fail closed
/// unless the provider reports a real count. The check occurs once at the final
/// identity boundary, so persistent zero adds at most 31 milliseconds per
/// opened file rather than multiplying across the read path.
#[cfg(windows)]
pub(crate) fn stable_windows_link_count(file: &File, path: &Path, label: &str) -> Result<u64> {
    stable_windows_link_count_with(
        || winapi_util::file::information(file).map(|information| information.number_of_links()),
        || std::thread::sleep(std::time::Duration::from_millis(1)),
        path,
        label,
    )
}

#[cfg(windows)]
fn stable_windows_link_count_with(
    query: impl FnMut() -> std::io::Result<u64>,
    wait: impl FnMut(),
    path: &Path,
    label: &str,
) -> Result<u64> {
    let link_count = retry_zero_windows_link_count(query, wait)
        .map_err(|source| CodegenError::io(&format!("inspect {label} link count"), path, source))?;
    if link_count == 0 {
        return Err(CodegenError::new(format!(
            "{label} `{}` repeatedly reported 0 hard links; alias safety could not be verified",
            path.display()
        )));
    }
    Ok(link_count)
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Read as _;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[cfg(windows)]
    #[test]
    fn zero_windows_link_counts_are_retried_but_never_accepted() {
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

        let error = stable_windows_link_count_with(
            || Ok(0),
            || {},
            Path::new("synthetic-file"),
            "test asset",
        )
        .expect_err("persistent zero must fail closed");
        assert!(error.to_string().contains("alias safety"));
    }

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "bir-rules-codegen-verified-file-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn verified_file_is_unread_until_path_and_handle_identity_pass() {
        let root = temp_root("identity");
        let path = root.join("asset.bin");
        fs::write(&path, b"official-bytes").unwrap();
        let calls = AtomicUsize::new(0);

        let mut verified = open_verified_regular_file(&path, "test asset", |resolved| {
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

        let error = open_verified_regular_file(&path, "test asset", |_| {
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

        let error = open_verified_regular_file(&alias, "test asset", |_| Ok(())).unwrap_err();
        assert!(error.to_string().contains("hard links"));
        fs::remove_dir_all(root).unwrap();
    }
}
