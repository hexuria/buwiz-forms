//! How much of the extracted corpus is actually executable.
//!
//! The v1 corpus is research evidence: 43 forms of fields, rules, calculations
//! and messages, transcribed from the official package with a source reference
//! per record. It is not executable. Only a v2 IR snapshot compiles into
//! `bir-rules`, and only a `reviewed` snapshot can ever be resolved at runtime.
//!
//! Nothing reported this gap, so it was invisible without ad-hoc scripting, and
//! "how far along are we" could only be answered anecdotally. The distance
//! between 2,007 extracted rules and the handful that execute is the single
//! most useful number for deciding scope, so it gets a command.
//!
//! This is a **measurement, not a gate**. It never fails; `status` is where
//! conditions live.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{CodegenError, Result};
use crate::files::read_bytes;
use crate::json::{JsonValue, parse_strict};
use crate::path::{canonical_repo_root, resolve_existing_under};

const FORMS_DIR: &str = "rules/forms";
const INDEX_PATH: &str = "rules/ir/v2/index.json";

#[derive(Clone, Debug)]
pub struct CoverageOptions {
    pub repo_root: PathBuf,
}

impl CoverageOptions {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FormCoverage {
    pub form_id: String,
    /// Research status from the form manifest. `complete` means the evidence
    /// inventory is complete or its gaps are explicit — never that the form is
    /// executable, filing-safe or release-ready.
    pub research_status: String,
    pub v1_fields: usize,
    pub v1_rules: usize,
    pub v1_calculations: usize,
    /// `None` when no v2 IR snapshot exists for this form at all.
    pub v2_review_status: Option<String>,
    pub v2_fields: usize,
    pub v2_rules: usize,
    pub v2_calculations: usize,
    /// True only when a snapshot is `reviewed`, which is the sole state that
    /// can be compiled into the production registry.
    pub runtime_resolvable: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CoverageReport {
    pub forms: Vec<FormCoverage>,
    pub form_count: usize,
    pub forms_with_v2_snapshot: usize,
    pub forms_runtime_resolvable: usize,
    pub v1_fields: usize,
    pub v1_rules: usize,
    pub v1_calculations: usize,
    pub v2_fields: usize,
    pub v2_rules: usize,
    pub v2_calculations: usize,
}

impl CoverageReport {
    /// Executable share of extracted validation rules, in percent.
    pub fn rule_coverage_percent(&self) -> f64 {
        percent(self.v2_rules, self.v1_rules)
    }

    /// Executable share of extracted calculations, in percent.
    pub fn calculation_coverage_percent(&self) -> f64 {
        percent(self.v2_calculations, self.v1_calculations)
    }
}

fn percent(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    (part as f64) * 100.0 / (whole as f64)
}

pub fn coverage(options: &CoverageOptions) -> Result<CoverageReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let forms_root = resolve_existing_under(&repo_root, FORMS_DIR, "forms directory")?;
    let snapshots = load_v2_snapshots(&repo_root)?;

    let mut forms = Vec::new();
    let mut directories: Vec<PathBuf> = std::fs::read_dir(&forms_root)
        .map_err(|source| CodegenError::io("read forms directory", &forms_root, source))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    directories.sort();

    for directory in directories {
        let Some(name) = directory.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let manifest = read_optional(&directory.join("manifest.json"))?;
        let Some(manifest) = manifest else {
            continue;
        };

        let form_id = string_at(&manifest, "form_id").unwrap_or(name).to_owned();
        let research_status = string_at(&manifest, "status")
            .unwrap_or("unknown")
            .to_owned();

        let v1_fields = count_in(&directory.join("fields.json"), "fields")?;
        let v1_rules = count_in(&directory.join("validations.json"), "rules")?;
        let v1_calculations = count_in(&directory.join("calculations.json"), "calculations")?;

        let snapshot = snapshots.get(&form_id);
        let (v2_review_status, v2_fields, v2_rules, v2_calculations) = match snapshot {
            Some(snapshot) => (
                Some(snapshot.review_status.clone()),
                snapshot.fields,
                snapshot.rules,
                snapshot.calculations,
            ),
            None => (None, 0, 0, 0),
        };
        let runtime_resolvable = v2_review_status.as_deref() == Some("reviewed");

        forms.push(FormCoverage {
            form_id,
            research_status,
            v1_fields,
            v1_rules,
            v1_calculations,
            v2_review_status,
            v2_fields,
            v2_rules,
            v2_calculations,
            runtime_resolvable,
        });
    }

    let sum = |select: fn(&FormCoverage) -> usize| forms.iter().map(select).sum();
    Ok(CoverageReport {
        form_count: forms.len(),
        forms_with_v2_snapshot: forms
            .iter()
            .filter(|form| form.v2_review_status.is_some())
            .count(),
        forms_runtime_resolvable: forms.iter().filter(|form| form.runtime_resolvable).count(),
        v1_fields: sum(|form| form.v1_fields),
        v1_rules: sum(|form| form.v1_rules),
        v1_calculations: sum(|form| form.v1_calculations),
        v2_fields: sum(|form| form.v2_fields),
        v2_rules: sum(|form| form.v2_rules),
        v2_calculations: sum(|form| form.v2_calculations),
        forms,
    })
}

struct SnapshotCounts {
    review_status: String,
    fields: usize,
    rules: usize,
    calculations: usize,
}

/// Maps a v1 form id to its v2 snapshot counts, when one exists.
fn load_v2_snapshots(repo_root: &Path) -> Result<BTreeMap<String, SnapshotCounts>> {
    let index_path = resolve_existing_under(repo_root, INDEX_PATH, "v2 index")?;
    let index = parse_strict(&read_bytes(&index_path)?, &index_path)?;
    let source_root = index_path
        .parent()
        .ok_or_else(|| CodegenError::new("v2 index has no parent directory"))?;

    let mut snapshots = BTreeMap::new();
    let Some(JsonValue::Array(entries)) = index.object().and_then(|index| index.get("snapshots"))
    else {
        return Ok(snapshots);
    };

    for entry in entries {
        let (Some(relative), Some(review_status)) =
            (string_at(entry, "path"), string_at(entry, "review_status"))
        else {
            continue;
        };
        let rule_set_path = resolve_existing_under(source_root, relative, "snapshot rule set")?;
        let rule_set = parse_strict(&read_bytes(&rule_set_path)?, &rule_set_path)?;

        // The v2 identity carries a form code; the v1 corpus keys by form id.
        // `legacy_v1` is the recorded bridge between them.
        let form_id = rule_set
            .object()
            .and_then(|rule_set| rule_set.get("legacy_v1"))
            .and_then(|legacy| string_at(legacy, "form_id"))
            .map(str::to_owned)
            .unwrap_or_else(|| relative.split('/').next().unwrap_or(relative).to_owned());

        snapshots.insert(
            form_id,
            SnapshotCounts {
                review_status: review_status.to_owned(),
                fields: array_len(&rule_set, "fields"),
                rules: array_len(&rule_set, "rules"),
                calculations: array_len(&rule_set, "calculations"),
            },
        );
    }
    Ok(snapshots)
}

fn read_optional(path: &Path) -> Result<Option<JsonValue>> {
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(parse_strict(&read_bytes(path)?, path)?))
}

fn count_in(path: &Path, key: &str) -> Result<usize> {
    Ok(read_optional(path)?
        .map(|document| array_len(&document, key))
        .unwrap_or_default())
}

fn array_len(value: &JsonValue, key: &str) -> usize {
    match value.object().and_then(|object| object.get(key)) {
        Some(JsonValue::Array(values)) => values.len(),
        _ => 0,
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
    use super::{CoverageOptions, coverage};

    fn landed() -> super::CoverageReport {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        coverage(&CoverageOptions::new(root)).expect("measure landed corpus")
    }

    #[test]
    fn landed_coverage_matches_the_audited_corpus_totals() {
        let report = landed();
        assert_eq!(report.form_count, 43);
        assert_eq!(report.v1_fields, 9_592);
        assert_eq!(report.v1_rules, 2_007);
        assert_eq!(report.v1_calculations, 623);
    }

    /// The number this command exists to surface. Not a threshold — if it rises
    /// because real work landed, update it and say so; if it rises because the
    /// measurement changed meaning, that is a bug.
    #[test]
    fn executable_coverage_is_a_small_fraction_of_extracted_evidence() {
        let report = landed();
        assert_eq!(report.forms_with_v2_snapshot, 1, "only 2550Q has a v2 IR");
        assert_eq!(
            report.forms_runtime_resolvable, 0,
            "no snapshot is `reviewed`, so nothing resolves in a production build"
        );
        assert!(report.rule_coverage_percent() < 5.0);
        assert!(report.calculation_coverage_percent() < 1.0);
    }
}
