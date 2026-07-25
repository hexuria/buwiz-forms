use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{CodegenError, Result};
use crate::path::{is_json_file, normalized_relative_path, reject_symlink_components};

static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    let mut file =
        File::open(path).map_err(|source| CodegenError::io("open file", path, source))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
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
    let mut files = BTreeMap::new();
    visit_files(root, root, &mut |path| {
        let relative = normalized_relative_path(root, path)?;
        files.insert(relative, read_bytes(path)?);
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
    if metadata.file_type().is_symlink() {
        return Err(CodegenError::new(format!(
            "refusing to walk symlink `{}`",
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
        let file_type = entry
            .file_type()
            .map_err(|source| CodegenError::io("read file type", &path, source))?;
        if file_type.is_symlink() {
            return Err(CodegenError::new(format!(
                "refusing to traverse symlink `{}`",
                path.display()
            )));
        }
        if file_type.is_dir() {
            visit_files(safety_root, &path, visitor)?;
        } else if file_type.is_file() {
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

pub fn write_tree_atomically(target: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let parent = target.parent().ok_or_else(|| {
        CodegenError::new(format!(
            "generated output `{}` has no parent directory",
            target.display()
        ))
    })?;
    fs::create_dir_all(parent)
        .map_err(|source| CodegenError::io("create generated-output parent", parent, source))?;

    if target.exists() {
        let metadata = fs::symlink_metadata(target)
            .map_err(|source| CodegenError::io("read generated-output metadata", target, source))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(CodegenError::new(format!(
                "generated output `{}` must be a real directory",
                target.display()
            )));
        }
    }

    let staging = unique_sibling(target, "staging")?;
    let backup = unique_sibling(target, "backup")?;
    fs::create_dir(&staging).map_err(|source| {
        CodegenError::io(
            "create generated-output staging directory",
            &staging,
            source,
        )
    })?;

    let populate_result = populate_tree(&staging, files);
    if let Err(error) = populate_result {
        remove_owned_tree(&staging)?;
        return Err(error);
    }

    if !target.exists() {
        fs::rename(&staging, target)
            .map_err(|source| CodegenError::io("install generated-output tree", target, source))?;
        return Ok(());
    }

    fs::rename(target, &backup).map_err(|source| {
        CodegenError::io("stage previous generated-output tree", target, source)
    })?;
    if let Err(source) = fs::rename(&staging, target) {
        let install_error = CodegenError::io("install generated-output tree", target, source);
        if let Err(rollback_source) = fs::rename(&backup, target) {
            return Err(CodegenError::with_source(
                format!(
                    "{install_error}; rollback from `{}` also failed",
                    backup.display()
                ),
                rollback_source,
            ));
        }
        remove_owned_tree(&staging)?;
        return Err(install_error);
    }

    remove_owned_tree(&backup)?;
    Ok(())
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

fn remove_owned_tree(path: &Path) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if !name.contains(".bir-rules-codegen-") {
        return Err(CodegenError::new(format!(
            "refusing to remove non-codegen path `{}`",
            path.display()
        )));
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(CodegenError::io(
                "read generated-output temporary tree metadata",
                path,
                source,
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "refusing to remove non-directory codegen path `{}`",
            path.display()
        )));
    }
    remove_owned_directory(path)
}

fn remove_owned_directory(path: &Path) -> Result<()> {
    let mut entries = fs::read_dir(path)
        .map_err(|source| CodegenError::io("read codegen temporary directory", path, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CodegenError::io("read codegen temporary entry", path, source))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let entry_path = entry.path();
        let file_type = entry.file_type().map_err(|source| {
            CodegenError::io("read codegen temporary file type", &entry_path, source)
        })?;
        if file_type.is_symlink() {
            return Err(CodegenError::new(format!(
                "refusing to remove symlink in codegen temporary tree `{}`",
                entry_path.display()
            )));
        }
        if file_type.is_dir() {
            remove_owned_directory(&entry_path)?;
        } else if file_type.is_file() {
            fs::remove_file(&entry_path).map_err(|source| {
                CodegenError::io("remove codegen temporary file", &entry_path, source)
            })?;
        } else {
            return Err(CodegenError::new(format!(
                "unsupported entry in codegen temporary tree `{}`",
                entry_path.display()
            )));
        }
    }

    fs::remove_dir(path)
        .map_err(|source| CodegenError::io("remove generated-output temporary tree", path, source))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::{read_tree, unique_sibling, write_tree_atomically};

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
        write_tree_atomically(&target, &first).expect("first write");

        let second = BTreeMap::from([("nested/keep.rs".to_owned(), b"after".to_vec())]);
        write_tree_atomically(&target, &second).expect("replacement");

        assert_eq!(read_tree(&target).expect("read output"), second);
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
}
