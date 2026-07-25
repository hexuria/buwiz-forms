use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{CodegenError, Result};

pub const DEFAULT_SOURCE_DIR: &str = "rules/ir/v2";
pub const DEFAULT_SCHEMA_DIR: &str = "rules/schema/v2";
pub const DEFAULT_OUTPUT_DIR: &str = "crates/bir-rules/src/generated";

pub fn discover_repo_root(start: &Path) -> Result<PathBuf> {
    let start = fs::canonicalize(start).map_err(|source| {
        CodegenError::io("canonicalize repository search start", start, source)
    })?;
    for candidate in start.ancestors() {
        if candidate.join("rules").is_dir() && candidate.join("crates/bir-rules").is_dir() {
            return Ok(candidate.to_path_buf());
        }
    }
    Err(CodegenError::new(format!(
        "could not find a repository root above `{}`",
        start.display()
    )))
}

pub fn canonical_repo_root(path: &Path) -> Result<PathBuf> {
    let root = fs::canonicalize(path)
        .map_err(|source| CodegenError::io("canonicalize repository root", path, source))?;
    let metadata = fs::metadata(&root)
        .map_err(|source| CodegenError::io("read repository-root metadata", &root, source))?;
    if !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "repository root `{}` is not a directory",
            root.display()
        )));
    }
    Ok(root)
}

pub fn validate_portable_relative(value: &str, label: &str) -> Result<Vec<String>> {
    if value.is_empty() {
        return Err(CodegenError::new(format!("{label} must not be empty")));
    }
    if value.contains('\\') {
        return Err(CodegenError::new(format!(
            "{label} `{value}` must use `/`, not `\\`"
        )));
    }
    if value.starts_with('/') || value.ends_with('/') {
        return Err(CodegenError::new(format!(
            "{label} `{value}` must be a normalized relative path"
        )));
    }

    let mut components = Vec::new();
    for component in value.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(CodegenError::new(format!(
                "{label} `{value}` contains a forbidden path component"
            )));
        }
        if component.contains(':') || component.contains('\0') {
            return Err(CodegenError::new(format!(
                "{label} `{value}` contains a non-portable path component"
            )));
        }
        components.push(component.to_owned());
    }
    Ok(components)
}

pub fn portable_join(root: &Path, value: &str, label: &str) -> Result<PathBuf> {
    let components = validate_portable_relative(value, label)?;
    let mut path = root.to_path_buf();
    for component in components {
        path.push(component);
    }
    Ok(path)
}

pub fn resolve_existing_under(root: &Path, relative: &str, label: &str) -> Result<PathBuf> {
    let candidate = portable_join(root, relative, label)?;
    reject_symlink_components(root, &candidate, label)?;
    let resolved = fs::canonicalize(&candidate)
        .map_err(|source| CodegenError::io(&format!("resolve {label}"), &candidate, source))?;
    ensure_under(root, &resolved, label)?;
    Ok(resolved)
}

pub fn resolve_output_under(root: &Path, relative: &str, label: &str) -> Result<PathBuf> {
    let candidate = portable_join(root, relative, label)?;
    let mut existing = candidate.as_path();
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| {
            CodegenError::new(format!(
                "{label} `{}` has no existing ancestor",
                candidate.display()
            ))
        })?;
    }
    reject_symlink_components(root, existing, label)?;
    let resolved_ancestor = fs::canonicalize(existing).map_err(|source| {
        CodegenError::io(&format!("resolve {label} ancestor"), existing, source)
    })?;
    ensure_under(root, &resolved_ancestor, label)?;
    Ok(candidate)
}

pub fn ensure_under(root: &Path, candidate: &Path, label: &str) -> Result<()> {
    if candidate == root || !candidate.starts_with(root) {
        return Err(CodegenError::new(format!(
            "{label} `{}` escapes repository root `{}`",
            candidate.display(),
            root.display()
        )));
    }
    Ok(())
}

pub fn reject_symlink_components(root: &Path, candidate: &Path, label: &str) -> Result<()> {
    if candidate == root {
        return Ok(());
    }
    ensure_under(root, candidate, label)?;
    let relative = candidate.strip_prefix(root).map_err(|_| {
        CodegenError::new(format!(
            "{label} `{}` is not under `{}`",
            candidate.display(),
            root.display()
        ))
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(value) => current.push(value),
            _ => {
                return Err(CodegenError::new(format!(
                    "{label} `{}` contains a forbidden component",
                    candidate.display()
                )));
            }
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(CodegenError::new(format!(
                    "{label} `{}` traverses symlink `{}`",
                    candidate.display(),
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => break,
            Err(source) => {
                return Err(CodegenError::io(
                    &format!("inspect {label} path component"),
                    &current,
                    source,
                ));
            }
        }
    }
    Ok(())
}

pub fn normalized_relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        CodegenError::new(format!(
            "path `{}` is not under `{}`",
            path.display(),
            root.display()
        ))
    })?;
    let mut output = String::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(CodegenError::new(format!(
                "path `{}` is not normalized",
                relative.display()
            )));
        };
        let component = component.to_str().ok_or_else(|| {
            CodegenError::new(format!("path `{}` is not valid UTF-8", relative.display()))
        })?;
        if !output.is_empty() {
            output.push('/');
        }
        output.push_str(component);
    }
    Ok(output)
}

pub fn is_json_file(path: &Path) -> bool {
    path.extension() == Some(OsStr::new("json"))
}

#[cfg(test)]
mod tests {
    use super::validate_portable_relative;

    #[test]
    fn portable_paths_reject_escape_and_platform_specific_forms() {
        for value in [
            "../outside",
            "inside/../outside",
            "/absolute",
            "C:/absolute",
            r"inside\windows",
            "inside//empty",
            "./inside",
        ] {
            assert!(
                validate_portable_relative(value, "test path").is_err(),
                "{value} should fail"
            );
        }
    }

    #[test]
    fn portable_paths_accept_normalized_repository_relative_paths() {
        assert_eq!(
            validate_portable_relative("2550q-v2024-p7.9.6/rule-set.json", "test path")
                .expect("valid path"),
            ["2550q-v2024-p7.9.6", "rule-set.json"]
        );
    }
}
