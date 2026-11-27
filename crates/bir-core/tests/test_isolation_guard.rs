//! No test anywhere in the repository may reach the person's real keychain
//! (service `com.buwiz.bir` and the others), real data folder or real logs.
//!
//! bir-core's test-build rule (`platform::keychain_disabled`,
//! `platform::data_dir`, `log_files::log_dir`) holds only where the tests
//! are built with bir-core's `test-support` feature or `cfg(test)`. This file
//! fails when that stops being true:
//! - a crate that uses bir-core does not give its tests `test-support`;
//! - a new keychain call site appears that this list has not reviewed;
//! - this test build itself does not have the keychain off.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

/// Every Cargo.toml in the repository that can use bir-core.
fn manifests(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(root.join("crates"))
        .expect("crates/")
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("Cargo.toml"))
        .filter(|path| path.is_file())
        .collect();
    found.push(root.join("nsh/Cargo.toml"));
    found.sort();
    found
}

/// The lines of one `[section]`, and of every section whose header ends with
/// `dependencies]` when `wanted` is `None`.
fn section_lines<'a>(text: &'a str, wanted: impl Fn(&str) -> bool) -> Vec<&'a str> {
    let mut inside = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = wanted(trimmed);
            continue;
        }
        if inside {
            lines.push(trimmed);
        }
    }
    lines
}

#[test]
fn every_crate_that_uses_bir_core_tests_with_its_test_build_rule() {
    let root = repo_root();
    let mut missing = Vec::new();
    for manifest in manifests(&root) {
        let text = fs::read_to_string(&manifest).expect("read Cargo.toml");
        let is_bir_core = text.contains("name = \"bir-core\"");
        let uses = section_lines(&text, |header| {
            header.ends_with("dependencies]") && !header.contains("dev-dependencies")
        })
        .iter()
        .any(|line| line.starts_with("bir-core"));
        if !uses && !is_bir_core {
            continue;
        }
        let tested = section_lines(&text, |header| header == "[dev-dependencies]")
            .iter()
            .any(|line| line.starts_with("bir-core") && line.contains("\"test-support\""));
        if !tested {
            missing.push(manifest.strip_prefix(&root).unwrap().display().to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "these crates' tests would reach the real keychain and data folder; add \
         `bir-core = {{ path = …, default-features = false, features = [\"test-support\"] }}` \
         under [dev-dependencies]: {missing:?}"
    );
}

/// Each reviewed keychain call site: the file, and how many times it opens an
/// entry. Every one is behind `platform::keychain_disabled()` (bir-core), a
/// `use_keychain` flag its callers set from it (bir-submit), or the SQLCipher
/// feature, which no build turns on.
const REVIEWED_KEYCHAIN_CALLS: &[(&str, usize)] = &[
    ("crates/bir-core/src/cloud_account.rs", 1),
    ("crates/bir-core/src/db/mod.rs", 2),
    ("crates/bir-core/src/dev_mode.rs", 2),
    ("crates/bir-core/src/email_sign_in.rs", 3),
    ("crates/bir-core/src/google_calendar.rs", 3),
    ("crates/bir-submit/src/secrets.rs", 1),
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn every_keychain_call_site_is_reviewed() {
    let root = repo_root();
    let mut files = Vec::new();
    for crate_dir in fs::read_dir(root.join("crates"))
        .expect("crates/")
        .filter_map(Result::ok)
    {
        rust_files(&crate_dir.path().join("src"), &mut files);
    }
    rust_files(&root.join("nsh/src"), &mut files);

    let mut found: Vec<(String, usize)> = files
        .iter()
        .filter_map(|path| {
            let text = fs::read_to_string(path).ok()?;
            // `keyring::Entry::new(…)`, or a bare `Entry::new("…")` after `use keyring::Entry`.
            let count = text
                .lines()
                .filter(|line| {
                    line.contains("keyring::Entry::new(")
                        || (line.contains(" Entry::new(\"") && text.contains("use keyring::Entry"))
                })
                .count();
            (count > 0).then(|| {
                let relative = path.strip_prefix(&root).unwrap();
                (relative.to_string_lossy().replace('\\', "/"), count)
            })
        })
        .collect();
    found.sort();
    let reviewed: Vec<(String, usize)> = REVIEWED_KEYCHAIN_CALLS
        .iter()
        .map(|(file, count)| ((*file).to_owned(), *count))
        .collect();
    assert_eq!(
        found, reviewed,
        "a keychain call site was added, moved or removed. Put it behind \
         `platform::keychain_disabled()` (always true in a test build), then update \
         REVIEWED_KEYCHAIN_CALLS"
    );
}

#[test]
fn this_test_build_has_the_keychain_off() {
    temp_env::with_var("BUWIZ_NO_KEYCHAIN", None::<&str>, || {
        assert!(
            bir_core::platform::keychain_disabled(),
            "bir-core's own integration tests must be built with test-support"
        );
    });
}
