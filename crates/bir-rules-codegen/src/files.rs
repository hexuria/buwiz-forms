use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use same_file::Handle;

use crate::error::{CodegenError, Result};
use crate::path::{
    ensure_under, is_json_file, is_symlink_or_reparse_point, normalized_relative_path,
    reject_symlink_components,
};
use crate::verified_file::open_verified_regular_file;

static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
thread_local! {
    static BEFORE_TREE_INSTALL_HOOK: std::cell::RefCell<Option<Box<dyn Fn(&Path)>>> =
        std::cell::RefCell::new(None);
}

pub fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    let mut file = open_verified_regular_file(path, "file", |_| Ok(()))?;
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

pub fn read_tree(root: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
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
        let mut verified = open_verified_regular_file(path, "tree file", |resolved| {
            ensure_under(&canonical_root, resolved, "resolved tree file")
        })?;
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

    use super::{
        BEFORE_TREE_INSTALL_HOOK, read_tree, rename_no_replace, unique_sibling,
        write_tree_atomically,
    };

    fn temporary_directory(label: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "bir-rules-codegen-{label}-{}-{}",
            std::process::id(),
            super::UNIQUE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("create test directory");
        directory
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

        assert_eq!(read_tree(&target).expect("read output"), second);
        let preserved = second_write
            .preserved_previous
            .expect("replacement preserves the previous output");
        assert_eq!(read_tree(&preserved).expect("read preserved output"), first);
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
            read_tree(&displaced).expect("read displaced original"),
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

        let error = read_tree(&root).expect_err("hard-linked tree entry must fail closed");
        assert!(error.to_string().contains("hard links"));
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
