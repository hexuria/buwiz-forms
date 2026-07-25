//! Atomic re-pin of a v2 snapshot's `source_set_sha256` and its declared
//! source hashes.
//!
//! Rolling the pin by hand is a 123-file, 246-site transaction.
//! `handoff.md:486-524` describes doing it with a 64-zero placeholder, a
//! deliberately-failed audit, and reading the correct digest out of the error
//! text. That is unnecessary: [`crate::audit::snapshot_source_digest`] nulls
//! `identity.source_set_sha256` and every embedded fixture pin *before*
//! hashing, so the digest does not depend on the current pin values and can be
//! computed directly.
//!
//! `handoff.md:524` requires that "a partial digest roll must fail rather than
//! be patched around". Nothing enforced that. This does: every file is staged
//! in memory, the site counts are asserted before anything is written, and a
//! failure mid-write restores every file already touched.
//!
//! Substitution is **textual**, not a JSON round-trip, so the roll changes only
//! the digests and leaves formatting alone. That keeps the diff reviewable and
//! keeps this command orthogonal to the canonical reformat.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::audit::snapshot_source_digest;
use crate::error::{CodegenError, Result};
use crate::files::read_bytes;
use crate::hash::sha256_hex;
use crate::json::{JsonValue, parse_strict};
use crate::path::{canonical_repo_root, resolve_existing_under};

const RULES_DIR: &str = "rules";
const SOURCE_DIR: &str = "rules/ir/v2";
const INDEX_PATH: &str = "rules/ir/v2/index.json";

#[derive(Clone, Debug)]
pub struct RollPinOptions {
    pub repo_root: PathBuf,
    pub rule_set_id: String,
    /// Report what would change without touching the working tree.
    pub dry_run: bool,
}

impl RollPinOptions {
    pub fn new(repo_root: impl Into<PathBuf>, rule_set_id: impl Into<String>) -> Self {
        Self {
            repo_root: repo_root.into(),
            rule_set_id: rule_set_id.into(),
            dry_run: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SourceRepin {
    pub source_id: String,
    pub path: String,
    pub previous_sha256: String,
    pub current_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RollPinReport {
    pub rule_set_id: String,
    pub source_repins: Vec<SourceRepin>,
    pub previous_source_set_sha256: String,
    pub source_set_sha256: String,
    pub files_touched: usize,
    pub pin_sites: usize,
    pub applied: bool,
}

impl RollPinReport {
    /// True when the corpus already agrees with itself and nothing was written.
    pub fn already_consistent(&self) -> bool {
        self.source_repins.is_empty() && self.previous_source_set_sha256 == self.source_set_sha256
    }
}

pub fn roll_pin(options: &RollPinOptions) -> Result<RollPinReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let rules_root = resolve_existing_under(&repo_root, RULES_DIR, "rules directory")?;
    let source_root = resolve_existing_under(&repo_root, SOURCE_DIR, "v2 source directory")?;
    let index_path = resolve_existing_under(&repo_root, INDEX_PATH, "v2 index")?;

    let index_text = read_text(&index_path)?;
    let index_json = parse_strict(index_text.as_bytes(), &index_path)?;

    let snapshot = index_json
        .object()
        .and_then(|index| index.get("snapshots"))
        .and_then(as_array)
        .and_then(|snapshots| {
            snapshots.iter().find(|snapshot| {
                string_at(snapshot, "rule_set_id") == Some(options.rule_set_id.as_str())
            })
        })
        .ok_or_else(|| {
            CodegenError::new(format!(
                "snapshot `{}` is not listed in {INDEX_PATH}",
                options.rule_set_id
            ))
        })?;

    let snapshot_relative = string_at(snapshot, "path").ok_or_else(|| {
        CodegenError::new(format!(
            "snapshot `{}` has no path in {INDEX_PATH}",
            options.rule_set_id
        ))
    })?;
    let rule_set_path =
        resolve_existing_under(&source_root, snapshot_relative, "snapshot rule set")?;

    let original_rule_set_text = read_text(&rule_set_path)?;
    let original_rule_set = parse_strict(original_rule_set_text.as_bytes(), &rule_set_path)?;

    let previous_digest = original_rule_set
        .object()
        .and_then(|rule_set| rule_set.get("identity"))
        .and_then(|identity| string_at(identity, "source_set_sha256"))
        .ok_or_else(|| {
            CodegenError::new(
                "rule set does not pin identity.source_set_sha256; refusing to roll a snapshot with no pin to replace",
            )
        })?
        .to_owned();

    // 1. Re-pin declared sources first: `sources[]` is part of the rule set's
    //    canonical bytes, so it must settle before the source-set digest is
    //    computed from it.
    let (rule_set_text, source_repins) =
        repin_declared_sources(&rules_root, &original_rule_set, original_rule_set_text)?;
    let rule_set_json = parse_strict(rule_set_text.as_bytes(), &rule_set_path)?;

    // 2. Compute the new digest directly. No placeholder, no failed audit.
    let fixture_relatives = fixture_paths(&rule_set_json)?;
    let mut fixture_texts = BTreeMap::new();
    let mut fixture_values = BTreeMap::new();
    for relative in &fixture_relatives {
        let path = resolve_existing_under(&rules_root, relative, "v2 fixture path")?;
        let text = read_text(&path)?;
        fixture_values.insert(relative.clone(), parse_strict(text.as_bytes(), &path)?);
        fixture_texts.insert(relative.clone(), (path, text));
    }
    let digest = snapshot_source_digest(&rule_set_json, &fixture_values)?;

    // 3. Stage every write, asserting the site count per file before anything
    //    touches the disk.
    let mut staged: Vec<(PathBuf, String)> = Vec::new();
    let mut pin_sites = 0usize;

    let (index_updated, index_sites) =
        substitute_exact(&index_text, &previous_digest, &digest, 1, INDEX_PATH)?;
    pin_sites += index_sites;
    staged.push((index_path.clone(), index_updated));

    let (rule_set_updated, rule_set_sites) = substitute_exact(
        &rule_set_text,
        &previous_digest,
        &digest,
        1,
        snapshot_relative,
    )?;
    pin_sites += rule_set_sites;
    staged.push((rule_set_path.clone(), rule_set_updated));

    for (relative, (path, text)) in &fixture_texts {
        let sites = text.matches(previous_digest.as_str()).count();
        // Two pins per evaluation fixture (`input.rule_set` and
        // `expected.report.rule_set`), three when it also carries a
        // `workflow_transition`.
        if !(2..=3).contains(&sites) {
            return Err(CodegenError::new(format!(
                "fixture `{relative}` carries {sites} source-set pin(s); expected 2 or 3"
            )));
        }
        pin_sites += sites;
        staged.push((
            path.clone(),
            text.replace(previous_digest.as_str(), &digest),
        ));
    }

    let report = RollPinReport {
        rule_set_id: options.rule_set_id.clone(),
        source_repins,
        previous_source_set_sha256: previous_digest.clone(),
        source_set_sha256: digest.clone(),
        files_touched: staged.len(),
        pin_sites,
        applied: false,
    };

    if report.already_consistent() {
        return Ok(report);
    }
    if options.dry_run {
        return Ok(report);
    }

    write_all_or_restore(staged)?;
    Ok(RollPinReport {
        applied: true,
        ..report
    })
}

/// Recomputes every declared source hash from disk and rewrites the changed
/// ones in place. Each hash is asserted to occur exactly once in the document,
/// so a substitution can never hit an unintended site.
fn repin_declared_sources(
    rules_root: &Path,
    rule_set: &JsonValue,
    mut text: String,
) -> Result<(String, Vec<SourceRepin>)> {
    let sources = rule_set
        .object()
        .and_then(|rule_set| rule_set.get("sources"))
        .and_then(as_array)
        .ok_or_else(|| CodegenError::new("rule set has no sources array"))?;

    let mut repins = Vec::new();
    for source in sources {
        let (Some(source_id), Some(relative), Some(declared)) = (
            string_at(source, "source_id"),
            string_at(source, "path"),
            string_at(source, "sha256"),
        ) else {
            return Err(CodegenError::new(
                "a declared source is missing source_id, path or sha256",
            ));
        };
        let corpus_relative = format!("{relative}");
        let path = resolve_existing_under(rules_root, &corpus_relative, "declared source")?;
        let current = sha256_hex(&read_bytes(&path)?);
        if current == declared {
            continue;
        }
        let (updated, _) = substitute_exact(&text, declared, &current, 1, relative)?;
        text = updated;
        repins.push(SourceRepin {
            source_id: source_id.to_owned(),
            path: relative.to_owned(),
            previous_sha256: declared.to_owned(),
            current_sha256: current,
        });
    }
    Ok((text, repins))
}

fn fixture_paths(rule_set: &JsonValue) -> Result<Vec<String>> {
    rule_set
        .object()
        .and_then(|rule_set| rule_set.get("fixtures"))
        .and_then(as_array)
        .map(|fixtures| {
            fixtures
                .iter()
                .filter_map(|fixture| fixture.as_str().map(str::to_owned))
                .collect()
        })
        .ok_or_else(|| CodegenError::new("rule set has no fixtures array"))
}

/// Replaces `from` with `to`, requiring exactly `expected` occurrences.
fn substitute_exact(
    text: &str,
    from: &str,
    to: &str,
    expected: usize,
    label: &str,
) -> Result<(String, usize)> {
    let occurrences = text.matches(from).count();
    if occurrences != expected {
        return Err(CodegenError::new(format!(
            "`{label}` contains {occurrences} occurrence(s) of `{from}`; expected {expected}"
        )));
    }
    Ok((text.replace(from, to), occurrences))
}

/// Writes every staged file, restoring all previously written files if any
/// write fails. A partial roll must fail rather than be patched around.
fn write_all_or_restore(staged: Vec<(PathBuf, String)>) -> Result<()> {
    let mut backups: Vec<(PathBuf, Vec<u8>)> = Vec::with_capacity(staged.len());
    for (path, _) in &staged {
        backups.push((path.clone(), read_bytes(path)?));
    }

    let mut written = 0usize;
    for (path, contents) in &staged {
        if let Err(source) = fs::write(path, contents.as_bytes()) {
            let failure = CodegenError::io("write rolled pin", path, source);
            let mut restore_failures = Vec::new();
            for (restore_path, original) in backups.iter().take(written) {
                if let Err(restore_error) = fs::write(restore_path, original) {
                    restore_failures.push(format!("{}: {restore_error}", restore_path.display()));
                }
            }
            if restore_failures.is_empty() {
                return Err(CodegenError::with_source(
                    format!(
                        "digest roll failed after {written} file(s); all were restored, the corpus is unchanged"
                    ),
                    failure,
                ));
            }
            return Err(CodegenError::with_source(
                format!(
                    "digest roll failed after {written} file(s) AND rollback failed for: {}. \
                     The corpus is in a partially rolled state and must be restored from git.",
                    restore_failures.join("; ")
                ),
                failure,
            ));
        }
        written += 1;
    }
    Ok(())
}

fn read_text(path: &Path) -> Result<String> {
    let bytes = read_bytes(path)?;
    String::from_utf8(bytes).map_err(|source| {
        CodegenError::with_source(format!("`{}` is not valid UTF-8", path.display()), source)
    })
}

fn as_array(value: &JsonValue) -> Option<&Vec<JsonValue>> {
    match value {
        JsonValue::Array(values) => Some(values),
        _ => None,
    }
}

fn string_at<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    value
        .object()
        .and_then(|object| object.get(key))
        .and_then(JsonValue::as_str)
}

#[cfg(test)]
mod tests {
    use super::{RollPinOptions, roll_pin, substitute_exact};

    fn landed_repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// A dry run against the landed corpus must enumerate the full transaction
    /// without touching anything: 123 files, 246 pin sites.
    #[test]
    fn dry_run_enumerates_the_whole_transaction_without_writing() {
        let mut options = RollPinOptions::new(landed_repo_root(), "2550q-v2024-p7.9.6.0");
        options.dry_run = true;
        let report = roll_pin(&options).expect("dry run");

        assert_eq!(report.files_touched, 123, "index + rule set + 121 fixtures");
        assert_eq!(report.pin_sites, 246, "1 + 1 + 119*2 + 2*3");
        assert!(!report.applied);
    }

    /// The corpus currently agrees with itself: the recomputed digest equals
    /// the pinned one and every declared source hash matches its file. If this
    /// fails, either the corpus changed or the digest computation drifted —
    /// investigate before rolling anything.
    #[test]
    fn landed_corpus_is_already_self_consistent() {
        let mut options = RollPinOptions::new(landed_repo_root(), "2550q-v2024-p7.9.6.0");
        options.dry_run = true;
        let report = roll_pin(&options).expect("dry run");

        assert_eq!(
            report.previous_source_set_sha256, report.source_set_sha256,
            "recomputed source-set digest differs from the pinned one"
        );
        assert!(
            report.source_repins.is_empty(),
            "declared sources drifted: {:?}",
            report.source_repins
        );
        assert!(report.already_consistent());
    }

    #[test]
    fn substitution_refuses_an_unexpected_site_count() {
        assert!(substitute_exact("a a", "a", "b", 1, "test").is_err());
        assert!(substitute_exact("a", "a", "b", 2, "test").is_err());
        let (text, count) = substitute_exact("x a y", "a", "b", 1, "test").expect("one site");
        assert_eq!(text, "x b y");
        assert_eq!(count, 1);
    }

    #[test]
    fn unknown_snapshot_is_rejected() {
        let options = RollPinOptions::new(landed_repo_root(), "no-such-snapshot");
        assert!(roll_pin(&options).is_err());
    }
}
