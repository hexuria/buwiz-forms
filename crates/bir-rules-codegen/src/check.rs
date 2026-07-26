use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{CodegenError, Result};
use crate::files::read_tree;
use crate::generate::{
    GenerateOptions, GenerationReport, audit_for_generation, build_generated_files,
    resolve_generated_output,
};

#[derive(Clone, Debug)]
pub struct CheckOptions {
    pub generate: GenerateOptions,
    pub run_runtime_tests: bool,
}

impl CheckOptions {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            generate: GenerateOptions::new(repo_root),
            run_runtime_tests: true,
        }
    }
}

pub fn check(options: &CheckOptions) -> Result<GenerationReport> {
    let first_audit = audit_for_generation(&options.generate)?;
    let first = build_generated_files(&first_audit)?;
    let second_audit = audit_for_generation(&options.generate)?;
    let second = build_generated_files(&second_audit)?;
    compare_file_trees("independent generation runs", &first.files, &second.files)?;

    let output = resolve_generated_output(&first_audit.repo_root, &options.generate.output_dir)?;
    let tracked = read_tree(&output)?;
    compare_file_trees("tracked generated output", &first.files, &tracked)?;

    run_rust_format_check(&first_audit.repo_root)?;
    if options.run_runtime_tests {
        run_runtime_tests(&first_audit.repo_root)?;
    }
    Ok(first)
}

fn compare_file_trees(
    label: &str,
    expected: &BTreeMap<String, Vec<u8>>,
    actual: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    if expected == actual {
        return Ok(());
    }
    let expected_paths = expected.keys().cloned().collect::<BTreeSet<_>>();
    let actual_paths = actual.keys().cloned().collect::<BTreeSet<_>>();
    let missing = expected_paths
        .difference(&actual_paths)
        .cloned()
        .collect::<Vec<_>>();
    let extra = actual_paths
        .difference(&expected_paths)
        .cloned()
        .collect::<Vec<_>>();
    let changed = expected_paths
        .intersection(&actual_paths)
        .filter(|path| expected.get(*path) != actual.get(*path))
        .cloned()
        .collect::<Vec<_>>();
    Err(CodegenError::new(format!(
        "{label} differs; missing: {missing:?}; extra: {extra:?}; byte-changed: {changed:?}"
    )))
}

fn cargo_executable() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

fn run_rust_format_check(repo_root: &Path) -> Result<()> {
    let output = Command::new(cargo_executable())
        .current_dir(repo_root)
        .args(["fmt", "--package", "bir-rules", "--", "--check"])
        .output()
        .map_err(|source| {
            CodegenError::with_source(
                "failed to start `cargo fmt --package bir-rules -- --check`",
                source,
            )
        })?;
    if output.status.success() {
        return Ok(());
    }

    Err(CodegenError::new(format!(
        "`cargo fmt --package bir-rules -- --check` failed with {}{}{}",
        output.status,
        command_output("stdout", &output.stdout),
        command_output("stderr", &output.stderr),
    )))
}

fn command_output(label: &str, bytes: &[u8]) -> String {
    if bytes.is_empty() {
        String::new()
    } else {
        format!("\n{label}:\n{}", String::from_utf8_lossy(bytes))
    }
}

fn run_runtime_tests(repo_root: &Path) -> Result<()> {
    let status = Command::new(cargo_executable())
        .current_dir(repo_root)
        .args(["test", "--locked", "-p", "bir-rules"])
        .status()
        .map_err(|source| {
            CodegenError::with_source("failed to start `cargo test --locked -p bir-rules`", source)
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "`cargo test --locked -p bir-rules` failed with {status}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{compare_file_trees, run_rust_format_check};

    #[test]
    fn exact_comparison_checks_paths_and_bytes() {
        let expected = BTreeMap::from([("one".to_owned(), b"same".to_vec())]);
        assert!(compare_file_trees("test", &expected, &expected).is_ok());

        let changed = BTreeMap::from([("one".to_owned(), b"changed".to_vec())]);
        let error =
            compare_file_trees("test", &expected, &changed).expect_err("byte drift must fail");
        assert!(error.message().contains("byte-changed: [\"one\"]"));
    }

    #[test]
    fn rust_format_check_rejects_unformatted_generated_module() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bir-rules-codegen-format-check-{}-{nonce}",
            std::process::id()
        ));
        let crate_root = root.join("crates/bir-rules");
        let generated = crate_root.join("src/generated");
        fs::create_dir_all(&generated).expect("create temporary generated source tree");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/bir-rules\"]\nresolver = \"2\"\n",
        )
        .expect("write temporary workspace manifest");
        fs::write(
            crate_root.join("Cargo.toml"),
            "[package]\nname = \"bir-rules\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .expect("write temporary crate manifest");
        fs::write(crate_root.join("src/lib.rs"), "mod generated;\n")
            .expect("write temporary crate root");
        fs::write(generated.join("mod.rs"), "mod registry;\n")
            .expect("write temporary generated module");
        fs::write(
            generated.join("registry.rs"),
            "pub fn generated_value()->u8{1}\n",
        )
        .expect("write unformatted generated Rust");

        let error = run_rust_format_check(&root)
            .expect_err("unformatted generated Rust must fail the normal format command");
        assert!(
            error
                .message()
                .contains("cargo fmt --package bir-rules -- --check")
        );

        fs::write(
            generated.join("registry.rs"),
            "pub fn generated_value() -> u8 {\n    1\n}\n",
        )
        .expect("write formatted generated Rust");
        run_rust_format_check(&root).expect("formatted generated Rust should pass");

        fs::remove_dir_all(&root).expect("remove temporary formatting workspace");
    }
}
