//! Build script for bir-core.
//!
//! Phase 1: Platform-specific library linking (unchanged).
//! Phase 2: Temporal Tax Engine snapshot compilation.
//!   - Reads canonical TOML files from data/temporal/
//!   - Validates cross-references (citations, formulas, rate tables)
//!   - Generates a JSON snapshot to $OUT_DIR/temporal_snapshot.json
//!   - Fails the build on validation errors

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;

fn main() {
    // ── Phase 1: Platform linking ──
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        println!("cargo:rustc-link-lib=dylib=user32");
        println!("cargo:rustc-link-lib=dylib=gdi32");
        println!("cargo:rustc-link-lib=dylib=crypt32");
        println!("cargo:rustc-link-lib=dylib=advapi32");
        println!("cargo:rustc-link-lib=dylib=ws2_32");
        println!("cargo:rustc-link-lib=dylib=userenv");
        println!("cargo:rustc-link-lib=dylib=bcrypt");
    }

    // ── Phase 2: Temporal Snapshot Compilation ──
    let data_dir = Path::new("data/temporal");
    if data_dir.exists() {
        println!("cargo:rerun-if-changed=data/temporal");
        compile_temporal_snapshot(data_dir);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Build-time data structures (mirrors runtime types)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    manifest: ManifestMeta,
    sources: ManifestSources,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManifestMeta {
    schema_version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManifestSources {
    citation_files: Vec<String>,
    era_files: Vec<String>,
    form_files: Vec<String>,
    rule_files: Vec<String>,
    rate_files: Vec<String>,
    formula_files: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CitationFile {
    citations: Vec<Citation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Citation {
    citation_id: String,
    kind: String,
    number: String,
    section: String,
    year: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct EraFile {
    era: Era,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Era {
    era_id: String,
    title: String,
    jurisdiction: String,
    effective_from_year: u16,
    #[serde(default)]
    effective_until_year: Option<u16>,
    #[serde(default)]
    is_overlay: bool,
    #[serde(default)]
    citations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FormFile {
    artifacts: Vec<FormArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FormArtifact {
    artifact_id: String,
    form_code: String,
    title: String,
    category: String,
    revision: String,
    effective_from: String,
    #[serde(default)]
    effective_until: Option<String>,
    #[serde(default = "default_lifecycle")]
    lifecycle: String,
    frequency: String,
    #[serde(default)]
    taxpayer_types: Vec<String>,
    #[serde(default)]
    classifications: Vec<String>,
    #[serde(default)]
    legal_citations: Vec<String>,
    #[serde(default)]
    schema_ref: Option<String>,
    #[serde(default)]
    template_ref: Option<String>,
    #[serde(default)]
    formula_ref: Option<String>,
    #[serde(default)]
    requires_vat: Option<bool>,
    #[serde(default)]
    withholding_trigger: Option<String>,
    #[serde(default)]
    excise_category: Option<String>,
    #[serde(default)]
    exclusive_group: Option<String>,
    #[serde(default)]
    exclusive_priority: u8,
}

fn default_lifecycle() -> String {
    "Active".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
struct RuleFile {
    rule: Rule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Rule {
    rule_id: String,
    title: String,
    era_id: String,
    effective_from: String,
    #[serde(default)]
    effective_until: Option<String>,
    phase: String,
    #[serde(default)]
    priority: i32,
    #[serde(default)]
    citations: Vec<String>,
    #[serde(default)]
    problem: String,
    #[serde(default)]
    solution: String,
    #[serde(default)]
    when: RuleCondition,
    #[serde(default)]
    mutations: Vec<RuleMutation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RuleCondition {
    #[serde(default)]
    all: Vec<String>,
    #[serde(default)]
    any: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuleMutation {
    #[serde(rename = "type")]
    mutation_type: String,
    reason: String,
    target: MutationTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MutationTarget {
    #[serde(default)]
    form_code: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    exclusive_group: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RateFile {
    rate_tables: Vec<RateTable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RateTable {
    table_id: String,
    title: String,
    jurisdiction: String,
    tax_type: String,
    effective_from: String,
    #[serde(default)]
    effective_until: Option<String>,
    #[serde(default)]
    rates: Vec<RateEntry>,
    #[serde(default)]
    citations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RateEntry {
    key: String,
    #[serde(default)]
    applies_when: Vec<String>,
    rate: String,
    #[serde(default)]
    threshold_min: Option<String>,
    #[serde(default)]
    threshold_max: Option<String>,
    #[serde(default)]
    fixed_amount: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FormulaFile {
    formulas: Vec<Formula>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Formula {
    formula_id: String,
    form_code: String,
    artifact_id: String,
    effective_from: String,
    #[serde(default)]
    effective_until: Option<String>,
    #[serde(default)]
    rate_table_refs: Vec<String>,
    #[serde(default)]
    input_fields: Vec<String>,
    #[serde(default)]
    output_fields: Vec<String>,
    #[serde(default)]
    citations: Vec<String>,
}

/// The compiled snapshot output written to JSON.
#[derive(Debug, Serialize)]
struct CompiledSnapshot {
    snapshot_id: String,
    content_hash: String,
    generated_at: String,
    schema_version: u32,
    eras: Vec<Era>,
    form_artifacts: Vec<FormArtifact>,
    rules: Vec<Rule>,
    rate_tables: Vec<RateTable>,
    formulas: Vec<Formula>,
    citations: Vec<Citation>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Compilation pipeline
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn compile_temporal_snapshot(data_dir: &Path) {
    let manifest_path = data_dir.join("snapshots/manifest.toml");
    if !manifest_path.exists() {
        println!(
            "cargo:warning=No temporal snapshot manifest found at {:?}",
            manifest_path
        );
        write_empty_snapshot();
        return;
    }

    let manifest_str = std::fs::read_to_string(&manifest_path).expect("Failed to read manifest");
    let manifest: Manifest = toml::from_str(&manifest_str).expect("Failed to parse manifest");

    let mut all_content = String::new();

    // ── Load citations ──
    let mut citations = Vec::new();
    for file in &manifest.sources.citation_files {
        let path = data_dir.join(file);
        let content = read_toml_file(&path);
        all_content.push_str(&content);
        let parsed: CitationFile = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));
        citations.extend(parsed.citations);
    }

    // ── Load eras ──
    let mut eras = Vec::new();
    for file in &manifest.sources.era_files {
        let path = data_dir.join(file);
        let content = read_toml_file(&path);
        all_content.push_str(&content);
        let parsed: EraFile = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));
        eras.push(parsed.era);
    }

    // ── Load form artifacts ──
    let mut form_artifacts = Vec::new();
    for file in &manifest.sources.form_files {
        let path = data_dir.join(file);
        let content = read_toml_file(&path);
        all_content.push_str(&content);
        let parsed: FormFile = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));
        form_artifacts.extend(parsed.artifacts);
    }

    // ── Load rules ──
    let mut rules = Vec::new();
    for file in &manifest.sources.rule_files {
        let path = data_dir.join(file);
        let content = read_toml_file(&path);
        all_content.push_str(&content);
        let parsed: RuleFile = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));
        rules.push(parsed.rule);
    }

    // ── Load rate tables ──
    let mut rate_tables = Vec::new();
    for file in &manifest.sources.rate_files {
        let path = data_dir.join(file);
        let content = read_toml_file(&path);
        all_content.push_str(&content);
        let parsed: RateFile = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));
        rate_tables.extend(parsed.rate_tables);
    }

    // ── Load formulas ──
    let mut formulas = Vec::new();
    for file in &manifest.sources.formula_files {
        let path = data_dir.join(file);
        let content = read_toml_file(&path);
        all_content.push_str(&content);
        let parsed: FormulaFile = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));
        formulas.extend(parsed.formulas);
    }

    // ── Validate ──
    let errors = validate(
        &citations,
        &eras,
        &form_artifacts,
        &rules,
        &rate_tables,
        &formulas,
    );
    if !errors.is_empty() {
        for err in &errors {
            println!("cargo:warning=Temporal validation error: {}", err);
        }
        panic!(
            "Temporal snapshot compilation failed with {} validation error(s).\nFix the TOML data files and rebuild.",
            errors.len()
        );
    }

    // ── Generate snapshot ──
    let content_hash = {
        let mut hasher = Sha256::new();
        hasher.update(all_content.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let short_hash = &content_hash[..8];
    let today = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .map(|_| "reproducible".to_string())
        .unwrap_or_else(|| {
            // Use a simple date string from the build environment
            format!(
                "{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            )
        });

    let snapshot = CompiledSnapshot {
        snapshot_id: format!("ttce.{}.{}", today, short_hash),
        content_hash,
        generated_at: today,
        schema_version: manifest.manifest.schema_version,
        eras,
        form_artifacts,
        rules,
        rate_tables,
        formulas,
        citations,
    };

    let json = serde_json::to_string_pretty(&snapshot).expect("Failed to serialize snapshot");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir).join("temporal_snapshot.json");
    std::fs::write(&out_path, &json).expect("Failed to write temporal_snapshot.json");

    println!(
        "cargo:warning=Temporal snapshot compiled: {} ({} eras, {} artifacts, {} rules, {} rate tables, {} formulas)",
        snapshot.snapshot_id,
        snapshot.eras.len(),
        snapshot.form_artifacts.len(),
        snapshot.rules.len(),
        snapshot.rate_tables.len(),
        snapshot.formulas.len(),
    );
}

fn write_empty_snapshot() {
    let snapshot = CompiledSnapshot {
        snapshot_id: "empty".into(),
        content_hash: "".into(),
        generated_at: "".into(),
        schema_version: 1,
        eras: vec![],
        form_artifacts: vec![],
        rules: vec![],
        rate_tables: vec![],
        formulas: vec![],
        citations: vec![],
    };
    let json = serde_json::to_string_pretty(&snapshot).unwrap();
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir).join("temporal_snapshot.json");
    std::fs::write(&out_path, &json).expect("Failed to write empty snapshot");
}

fn read_toml_file(path: &Path) -> String {
    println!("cargo:rerun-if-changed={}", path.display());
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Build-time validation (mirrors runtime validator.rs)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn validate(
    citations: &[Citation],
    eras: &[Era],
    artifacts: &[FormArtifact],
    rules: &[Rule],
    rate_tables: &[RateTable],
    formulas: &[Formula],
) -> Vec<String> {
    let mut errors = Vec::new();

    let cit_set: HashSet<&str> = citations.iter().map(|c| c.citation_id.as_str()).collect();
    let form_code_set: HashSet<&str> = artifacts.iter().map(|a| a.form_code.as_str()).collect();
    let formula_set: HashSet<&str> = formulas.iter().map(|f| f.formula_id.as_str()).collect();
    let rate_set: HashSet<&str> = rate_tables.iter().map(|r| r.table_id.as_str()).collect();

    // Check duplicate IDs
    check_dups(
        &mut errors,
        citations.iter().map(|c| c.citation_id.as_str()),
        "Citation",
    );
    check_dups(&mut errors, eras.iter().map(|e| e.era_id.as_str()), "Era");
    check_dups(
        &mut errors,
        artifacts.iter().map(|a| a.artifact_id.as_str()),
        "FormArtifact",
    );
    check_dups(
        &mut errors,
        rules.iter().map(|r| r.rule_id.as_str()),
        "Rule",
    );
    check_dups(
        &mut errors,
        rate_tables.iter().map(|r| r.table_id.as_str()),
        "RateTable",
    );
    check_dups(
        &mut errors,
        formulas.iter().map(|f| f.formula_id.as_str()),
        "Formula",
    );

    // Check citation references on artifacts
    for art in artifacts {
        for cit_id in &art.legal_citations {
            if !cit_set.contains(cit_id.as_str()) {
                errors.push(format!(
                    "Artifact '{}' references unknown citation '{}'",
                    art.artifact_id, cit_id
                ));
            }
        }
    }

    // Check citation references on rules
    for rule in rules {
        for cit_id in &rule.citations {
            if !cit_set.contains(cit_id.as_str()) {
                errors.push(format!(
                    "Rule '{}' references unknown citation '{}'",
                    rule.rule_id, cit_id
                ));
            }
        }
    }

    // Check rule mutation targets
    for rule in rules {
        for mutation in &rule.mutations {
            if let Some(ref fc) = mutation.target.form_code {
                if !form_code_set.contains(fc.as_str()) {
                    errors.push(format!(
                        "Rule '{}' targets unknown form code '{}'",
                        rule.rule_id, fc
                    ));
                }
            }
        }
    }

    // Check formula references on artifacts
    for art in artifacts {
        if let Some(ref fref) = art.formula_ref {
            if !fref.is_empty() && !formula_set.contains(fref.as_str()) {
                errors.push(format!(
                    "Artifact '{}' references unknown formula '{}'",
                    art.artifact_id, fref
                ));
            }
        }
    }

    // Check rate table references in formulas
    for formula in formulas {
        for rref in &formula.rate_table_refs {
            if !rate_set.contains(rref.as_str()) {
                errors.push(format!(
                    "Formula '{}' references unknown rate table '{}'",
                    formula.formula_id, rref
                ));
            }
        }
    }

    // Check overlapping primary eras
    let primary_eras: Vec<&Era> = eras.iter().filter(|e| !e.is_overlay).collect();
    for (i, a) in primary_eras.iter().enumerate() {
        for b in primary_eras.iter().skip(i + 1) {
            let a_end = a.effective_until_year.unwrap_or(u16::MAX);
            let b_end = b.effective_until_year.unwrap_or(u16::MAX);
            if a.effective_from_year <= b_end && b.effective_from_year <= a_end {
                errors.push(format!(
                    "Primary eras '{}' and '{}' overlap",
                    a.era_id, b.era_id
                ));
            }
        }
    }

    errors
}

fn check_dups<'a>(errors: &mut Vec<String>, ids: impl Iterator<Item = &'a str>, kind: &str) {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            errors.push(format!("Duplicate {} id: '{}'", kind, id));
        }
    }
}
