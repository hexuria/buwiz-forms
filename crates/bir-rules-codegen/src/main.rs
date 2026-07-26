use std::env;
use std::error::Error as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use bir_rules_codegen::{
    AcquireEvidenceVaultOptions, AuditOptions, BuildBindingsOptions, CheckOptions, CodegenError,
    CoverageOptions, DiscoverEvidenceVaultSourcesOptions, EvidenceCaptureOperatingSystem,
    EvidenceCaptureProvenance, FormIntegrationOptions, GenerateOptions, OperatorCensusOptions,
    ProjectStaticSurfaceOptions, ReconciliationOptions, Result, RollPinOptions,
    ScaffoldEvidenceReviewLedgerOptions, StatusOptions, ValidateV1Options,
    VerifyEvidenceVaultSourceMapOptions, WriteEvidenceVaultCaptureMetadataOptions,
    acquire_evidence_vault, audit, build_2550q_bindings, check, coverage,
    discover_evidence_vault_sources, generate, integrate_form,
    load_evidence_review_scaffold_request, operator_census, project_2550q_static_surface,
    reconciliation, roll_all_pins, roll_pin, scaffold_evidence_review_ledger, status, validate_v1,
    verify_evidence_vault_source_map, write_evidence_vault_capture_metadata,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            let mut source = error.source();
            while let Some(cause) = source {
                eprintln!("caused by: {cause}");
                source = cause.source();
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut arguments = env::args().skip(1);
    let command = arguments.next().ok_or_else(usage_error)?;
    if command == "--help" || command == "-h" || command == "help" {
        print_usage();
        return Ok(());
    }
    if command == "acquire-evidence-vault" {
        return run_acquire_evidence_vault(arguments);
    }
    if command == "discover-evidence-vault-sources" {
        return run_discover_evidence_vault_sources(arguments);
    }
    if command == "verify-evidence-vault-source-map" {
        return run_verify_evidence_vault_source_map(arguments);
    }
    if command == "write-evidence-capture-metadata" {
        return run_write_evidence_capture_metadata(arguments);
    }
    if command == "integrate-form" {
        return run_integrate_form(arguments);
    }
    if command == "scaffold-evidence-review-ledger" {
        return run_scaffold_evidence_review_ledger(arguments);
    }
    if matches!(
        command.as_str(),
        "verify-evidence" | "import-evidence" | "stage-form"
    ) {
        return bir_rules_codegen::run_evidence_command(&command, arguments);
    }
    if matches!(
        command.as_str(),
        "stage-evidence-packet-review"
            | "build-evidence-packet"
            | "build-evidence-packet-set"
            | "check-evidence-packet-set"
    ) {
        return bir_rules_codegen::run_evidence_set_command(&command, arguments);
    }
    if !matches!(
        command.as_str(),
        "audit"
            | "generate"
            | "check"
            | "validate-v1"
            | "status"
            | "build-2550q-bindings"
            | "project-2550q"
            | "roll-pin"
            | "coverage"
            | "operator-census"
            | "reconciliation"
    ) {
        return Err(CodegenError::new(format!(
            "unknown command `{command}`\n\n{}",
            usage()
        )));
    }

    let mut repo_root = None;
    let mut source_dir = None;
    let mut schema_dir = None;
    let mut output_dir = None;
    let mut rules_dir = None;
    let mut output_file = None;
    let mut rule_set_id = None;
    let mut all_rule_sets = false;
    let mut dry_run = false;
    let mut staging_root = None;
    let mut json_output = false;
    let mut require_promotion = false;
    let mut boundaries_only = false;
    let mut skip_runtime_tests = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--repo-root" => repo_root = Some(next_value(&mut arguments, "--repo-root")?),
            "--source-dir" => source_dir = Some(next_value(&mut arguments, "--source-dir")?),
            "--schema-dir" => schema_dir = Some(next_value(&mut arguments, "--schema-dir")?),
            "--output-dir" => output_dir = Some(next_value(&mut arguments, "--output-dir")?),
            "--rules-dir" if command == "validate-v1" => {
                rules_dir = Some(next_value(&mut arguments, "--rules-dir")?);
            }
            "--output" if command == "build-2550q-bindings" => {
                output_file = Some(next_value(&mut arguments, "--output")?);
            }
            "--rule-set-id"
                if matches!(
                    command.as_str(),
                    "audit" | "generate" | "check" | "roll-pin"
                ) =>
            {
                rule_set_id = Some(next_value(&mut arguments, "--rule-set-id")?);
            }
            "--all" if command == "roll-pin" => all_rule_sets = true,
            "--dry-run" if command == "roll-pin" => dry_run = true,
            "--staging-root" if command == "project-2550q" => {
                staging_root = Some(next_value(&mut arguments, "--staging-root")?);
            }
            "--json"
                if matches!(
                    command.as_str(),
                    "validate-v1" | "status" | "coverage" | "operator-census" | "reconciliation"
                ) =>
            {
                json_output = true;
            }
            "--require-promotion" if command == "status" => require_promotion = true,
            "--boundaries-only" if command == "status" => boundaries_only = true,
            "--skip-runtime-tests" if command == "check" => skip_runtime_tests = true,
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => {
                return Err(CodegenError::new(format!(
                    "unknown argument `{other}` for `{command}`"
                )));
            }
        }
    }
    if require_promotion && boundaries_only {
        return Err(CodegenError::new(
            "`status --require-promotion` cannot be combined with `--boundaries-only`",
        ));
    }

    let repo_root = match repo_root {
        Some(path) => PathBuf::from(path),
        None => bir_rules_codegen::discover_default_repo_root()?,
    };
    let v2_options_present = source_dir.is_some() || schema_dir.is_some() || output_dir.is_some();
    let mut audit_options = AuditOptions::new(&repo_root);
    if let Some(source_dir) = source_dir {
        audit_options.source_dir = source_dir;
    }
    if let Some(schema_dir) = schema_dir {
        audit_options.schema_dir = schema_dir;
    }

    match command.as_str() {
        "operator-census" => {
            if v2_options_present {
                return Err(CodegenError::new(
                    "`operator-census` reads the tracked corpus and does not accept source/schema/output options",
                ));
            }
            let report = operator_census(&OperatorCensusOptions::new(&repo_root))?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|source| {
                        CodegenError::with_source("serialize operator census", source)
                    })?
                );
            } else {
                println!(
                    "operator census: {} v2 snapshot(s), {}/{} field(s), {}/{} validation(s), {}/{} calculation(s) structurally represented",
                    report.v2_forms,
                    report.v2_fields,
                    report.v1_fields,
                    report.v2_validation_rules,
                    report.v1_validation_rules,
                    report.v2_calculations,
                    report.v1_calculations,
                );
                println!(
                    "untranslated: {} field(s), {} validation(s), {} calculation(s)",
                    report.untranslated_fields,
                    report.untranslated_validation_rules,
                    report.untranslated_calculations,
                );
            }
        }
        "reconciliation" => {
            if v2_options_present {
                return Err(CodegenError::new(
                    "`reconciliation` reads the tracked corpus and does not accept source/schema/output options",
                ));
            }
            let report = reconciliation(&ReconciliationOptions::new(&repo_root))?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|source| {
                        CodegenError::with_source("serialize reconciliation report", source)
                    })?
                );
            } else {
                println!(
                    "reconciliation: {}/{} v2 form(s) complete for the candidate library",
                    report.complete_forms, report.forms_with_v2_snapshot
                );
                println!(
                    "{} legacy record(s): {} represented, {} intentionally non-runtime, {} unresolved, {} unclassified",
                    report.legacy_records,
                    report.represented_records,
                    report.intentionally_non_runtime_records,
                    report.unresolved_records,
                    report.unclassified_records,
                );
            }
        }
        "coverage" => {
            let report = coverage(&CoverageOptions::new(&repo_root))?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|source| {
                        CodegenError::with_source("serialize coverage report", source)
                    })?
                );
            } else {
                println!(
                    "{:<16}{:>7}{:>7}{:>7}   {:<11}{:>7}{:>7}",
                    "form", "fields", "rules", "calcs", "v2", "exec-r", "exec-c"
                );
                for form in &report.forms {
                    println!(
                        "{:<16}{:>7}{:>7}{:>7}   {:<11}{:>7}{:>7}",
                        form.form_id,
                        form.v1_fields,
                        form.v1_rules,
                        form.v1_calculations,
                        form.v2_review_status.as_deref().unwrap_or("-"),
                        form.v2_rules_executable,
                        form.v2_calculations_executable,
                    );
                }
                println!();
                println!(
                    "{} form(s); {} with a v2 snapshot; {} resolvable at runtime",
                    report.form_count,
                    report.forms_with_v2_snapshot,
                    report.forms_runtime_resolvable
                );
                println!(
                    "executable rules       {}/{} ({:.1}%)",
                    report.v2_rules_executable,
                    report.v1_rules,
                    report.rule_coverage_percent()
                );
                println!(
                    "executable calculations {}/{} ({:.1}%)",
                    report.v2_calculations_executable,
                    report.v1_calculations,
                    report.calculation_coverage_percent()
                );
                println!(
                    "represented rules      {}/{} ({:.1}%)",
                    report.v2_rules,
                    report.v1_rules,
                    report.rule_representation_percent()
                );
                println!(
                    "represented calculations {}/{} ({:.1}%)",
                    report.v2_calculations,
                    report.v1_calculations,
                    report.calculation_representation_percent()
                );
            }
        }
        "status" => {
            if v2_options_present {
                return Err(CodegenError::new(
                    "`status` reads the tracked corpus and does not accept source/schema/output options",
                ));
            }
            let mut options = StatusOptions::new(&repo_root);
            options.boundaries_only = boundaries_only;
            let report = status(&options)?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|source| {
                        CodegenError::with_source("serialize status report", source)
                    })?
                );
            } else {
                println!(
                    "validation-rules library status (anchor {}){}",
                    report.rule_set_id,
                    if boundaries_only {
                        " — boundaries only"
                    } else if require_promotion {
                        " — promotion required"
                    } else {
                        " — active library"
                    }
                );
                for criterion in &report.criteria {
                    println!(
                        "  [{}] {:<38} {:?}  {}",
                        if criterion.met { "x" } else { " " },
                        criterion.id,
                        criterion.kind,
                        criterion.detail
                    );
                }
            }
            if !report.boundaries_held() {
                return Err(CodegenError::new(
                    "a production boundary is no longer closed; this is not unfinished library work",
                ));
            }
            if boundaries_only {
                return Ok(());
            }
            if !report.active_library_complete() {
                let open: Vec<&str> = report
                    .criteria
                    .iter()
                    .filter(|criterion| {
                        criterion.kind == bir_rules_codegen::CriterionKind::ActiveLibrary
                            && !criterion.met
                    })
                    .map(|criterion| criterion.id)
                    .collect();
                return Err(CodegenError::new(format!(
                    "active library incomplete; open criteria: {}",
                    open.join(", ")
                )));
            }
            if require_promotion && !report.deferred_promotion_complete() {
                let open: Vec<&str> = report
                    .criteria
                    .iter()
                    .filter(|criterion| {
                        criterion.kind == bir_rules_codegen::CriterionKind::DeferredPromotion
                            && !criterion.met
                    })
                    .map(|criterion| criterion.id)
                    .collect();
                return Err(CodegenError::new(format!(
                    "promotion incomplete; open deferred criteria: {}",
                    open.join(", ")
                )));
            }
        }
        "validate-v1" => {
            if v2_options_present {
                return Err(CodegenError::new(
                    "`validate-v1` audits the v1 corpus and does not accept v2 source/schema/output options",
                ));
            }
            let mut options = ValidateV1Options::new(&repo_root);
            if let Some(rules_dir) = rules_dir {
                options.rules_dir = rules_dir;
            }
            let report = validate_v1(&options)?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|source| {
                        CodegenError::with_source("serialize v1 corpus report", source)
                    })?
                );
            } else {
                println!(
                    "v1 corpus audit passed: {} form(s), {} JSON file(s) ({} v2), \
                     {} field(s), {} validation(s), {} calculation(s), \
                     {} negative fixture(s), {} schema document(s)",
                    report.forms_audited,
                    report.total_json_files,
                    report.v2_json_files,
                    report.fields,
                    report.validations,
                    report.calculations,
                    report.negative_fixtures,
                    report.schema_documents,
                );
            }
        }
        "build-2550q-bindings" => {
            if v2_options_present {
                return Err(CodegenError::new(
                    "`build-2550q-bindings` rebuilds a tracked fixture and does not accept v2 source/schema/output options",
                ));
            }
            let mut options = BuildBindingsOptions::new(&repo_root);
            if let Some(output_file) = output_file {
                options.output_path = output_file;
            }
            let report = build_2550q_bindings(&options)?;
            println!(
                "Wrote value-free 2550Q serialization binding inventory: {}",
                report.output_path.display()
            );
            println!("SHA-256: {}", report.sha256);
        }
        "project-2550q" => {
            if v2_options_present {
                return Err(CodegenError::new(
                    "`project-2550q` rewrites a tracked rule set and its fixtures and does not accept v2 source/schema/output options",
                ));
            }
            let report = {
                let mut options = ProjectStaticSurfaceOptions::new(&repo_root);
                options.staging_root = staging_root;
                project_2550q_static_surface(&options)?
            };
            println!(
                "Updated 2550Q v2 static projections: {} executable raw field(s) emitted, \
                 {} identity-only documented control(s) left unprojected, {} total field(s), \
                 and {} fixture(s) rewritten.",
                report.executable_raw_field_count,
                report.documented_only_control_count,
                report.total_field_count,
                report.fixture_count,
            );
            println!("Rule set: {}", report.rule_set_path.display());
        }
        "roll-pin" => {
            if v2_options_present {
                return Err(CodegenError::new(
                    "`roll-pin` rewrites tracked pins and does not accept source/schema/output options",
                ));
            }
            if all_rule_sets {
                if rule_set_id.is_some() {
                    return Err(CodegenError::new(
                        "`roll-pin --all` cannot be combined with `--rule-set-id`",
                    ));
                }
                if !dry_run {
                    return Err(CodegenError::new(
                        "`roll-pin --all` is verification-only and requires `--dry-run`",
                    ));
                }
                let report = roll_all_pins(&repo_root)?;
                for snapshot in &report.snapshots {
                    println!(
                        "snapshot {}: {}",
                        snapshot.rule_set_id,
                        if snapshot.already_consistent() {
                            "consistent"
                        } else {
                            "digest roll required"
                        }
                    );
                }
                if !report.all_consistent {
                    return Err(CodegenError::new(
                        "one or more snapshots require an individually reviewed atomic digest roll",
                    ));
                }
                println!(
                    "all {} snapshot digest transaction(s) are consistent",
                    report.snapshots.len()
                );
                return Ok(());
            }
            let rule_set_id = rule_set_id.ok_or_else(|| {
                CodegenError::new("`roll-pin` requires --rule-set-id <id> or --all --dry-run")
            })?;
            let mut options = RollPinOptions::new(&repo_root, rule_set_id);
            options.dry_run = dry_run;
            let report = roll_pin(&options)?;
            println!("snapshot {}", report.rule_set_id);
            for repin in &report.source_repins {
                println!(
                    "  source {} ({}): {} -> {}",
                    repin.source_id, repin.path, repin.previous_sha256, repin.current_sha256
                );
            }
            println!(
                "  source_set_sha256: {} -> {}",
                report.previous_source_set_sha256, report.source_set_sha256
            );
            println!(
                "  {} file(s), {} pin site(s)",
                report.files_touched, report.pin_sites
            );
            if report.already_consistent() {
                println!("  already consistent; nothing written");
            } else if report.applied {
                println!("  APPLIED — re-run `audit` to confirm");
            } else {
                println!("  dry run; nothing written");
            }
        }
        "audit" => {
            if output_dir.is_some() || skip_runtime_tests {
                return Err(CodegenError::new(
                    "`audit` does not accept output/test options",
                ));
            }
            let report = audit(&audit_options)?;
            let selected = rule_set_id
                .as_deref()
                .map(|rule_set_id| report.require_rule_set(rule_set_id))
                .transpose()?;
            println!(
                "v2 audit passed: {} snapshot(s), schema {}, source {}",
                report.snapshot_count(),
                report.schema_digest(),
                report.normalized_source_digest()
            );
            if let Some(selected) = selected {
                println!(
                    "aggregate audit passed; selected snapshot {} ({}/{}/{}) is present as {}",
                    selected.rule_set_id(),
                    selected.form_code(),
                    selected.form_revision(),
                    selected.official_package_version(),
                    selected.review_status(),
                );
            }
        }
        "generate" => {
            if skip_runtime_tests {
                return Err(CodegenError::new(
                    "`generate` does not accept --skip-runtime-tests",
                ));
            }
            let mut options = GenerateOptions::new(&repo_root);
            options.audit = audit_options;
            options.required_rule_set_id = rule_set_id.clone();
            if let Some(output_dir) = output_dir {
                options.output_dir = output_dir;
            }
            let report = generate(&options)?;
            println!(
                "generated {} file(s) from {} reviewed and {} test-only candidate snapshot(s); output {}",
                report.files.len(),
                report.reviewed_snapshot_count,
                report.candidate_snapshot_count,
                report.generated_output_digest
            );
            if let Some(rule_set_id) = rule_set_id.as_deref() {
                println!(
                    "aggregate generation passed; selected snapshot {rule_set_id} was present"
                );
            }
        }
        "check" => {
            let mut options = CheckOptions::new(&repo_root);
            options.generate.audit = audit_options;
            options.generate.required_rule_set_id = rule_set_id.clone();
            if let Some(output_dir) = output_dir {
                options.generate.output_dir = output_dir;
            }
            options.run_runtime_tests = !skip_runtime_tests;
            let report = check(&options)?;
            println!(
                "rules check passed: {} file(s), output {}",
                report.files.len(),
                report.generated_output_digest
            );
            if let Some(rule_set_id) = rule_set_id.as_deref() {
                println!("aggregate check passed; selected snapshot {rule_set_id} was present");
            }
        }
        _ => unreachable!("command was validated"),
    }
    Ok(())
}

fn next_value(arguments: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    arguments
        .next()
        .ok_or_else(|| CodegenError::new(format!("{flag} requires a value")))
}

fn run_discover_evidence_vault_sources(mut arguments: impl Iterator<Item = String>) -> Result<()> {
    let mut repo_root = None;
    let mut output_path = None;
    let mut search_roots = Vec::new();
    let mut dry_run = false;
    let mut json_output = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--repo-root" => set_once_cli_argument(
                &mut repo_root,
                next_value(&mut arguments, "--repo-root")?,
                "--repo-root",
            )?,
            "--output" => set_once_cli_argument(
                &mut output_path,
                next_value(&mut arguments, "--output")?,
                "--output",
            )?,
            "--search-root" => {
                search_roots.push(PathBuf::from(next_value(&mut arguments, "--search-root")?));
            }
            "--dry-run" if !dry_run => dry_run = true,
            "--dry-run" => {
                return Err(CodegenError::new("--dry-run may be provided only once"));
            }
            "--json" if !json_output => json_output = true,
            "--json" => {
                return Err(CodegenError::new("--json may be provided only once"));
            }
            "--help" | "-h" => {
                println!(
                    "{}",
                    [
                        "Usage: bir-rules-codegen discover-evidence-vault-sources \\",
                        "  --output FRESH-EXTERNAL-FILE [--search-root EXTERNAL-DIR ...] \\",
                        "  [--repo-root PATH] [--dry-run] [--json]",
                    ]
                    .join("\n")
                );
                return Ok(());
            }
            other => {
                return Err(CodegenError::new(format!(
                    "unknown argument `{other}` for `discover-evidence-vault-sources`"
                )));
            }
        }
    }
    let repo_root = match repo_root {
        Some(path) => PathBuf::from(path),
        None => bir_rules_codegen::discover_default_repo_root()?,
    };
    let output_path = output_path.ok_or_else(|| {
        CodegenError::new("`discover-evidence-vault-sources` requires --output FILE")
    })?;
    let mut options = DiscoverEvidenceVaultSourcesOptions::new(repo_root, output_path);
    options.search_roots = search_roots;
    options.dry_run = dry_run;
    let report = match discover_evidence_vault_sources(&options) {
        Ok(report) => report,
        Err(error) => {
            if json_output && let Some(unresolved) = error.unresolved_report() {
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(unresolved).map_err(|source| {
                        CodegenError::with_source(
                            "serialize unresolved evidence source discovery report",
                            source,
                        )
                    })?
                );
            }
            return Err(CodegenError::with_source(
                "evidence vault source discovery did not produce a complete source map",
                error,
            ));
        }
    };
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|source| {
                CodegenError::with_source("serialize evidence source discovery report", source)
            })?
        );
    } else {
        println!(
            "{} evidence source map: {} manifest(s), {} acquirable declaration(s), {} unique content file(s), {} deduplicated declaration(s), map {}",
            if report.written {
                "wrote"
            } else {
                "verified dry-run"
            },
            report.manifest_count,
            report.acquirable_asset_count,
            report.unique_content_count,
            report.deduplicated_asset_count,
            report.source_map_sha256,
        );
        println!("source map: {}", report.output_path.display());
        if !report.rejected_candidates.is_empty() {
            println!(
                "rejected candidates: {} (none were mapped)",
                report.rejected_candidates.len()
            );
        }
    }
    Ok(())
}

fn run_integrate_form(mut arguments: impl Iterator<Item = String>) -> Result<()> {
    let mut repo_root = None;
    let mut staging_root = None;
    let mut reviewed_packet_dir = None;
    let mut review_ledger_path = None;
    let mut rule_set_id = None;
    let mut apply = false;
    let mut json_output = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--repo-root" => set_once_cli_argument(
                &mut repo_root,
                next_value(&mut arguments, "--repo-root")?,
                "--repo-root",
            )?,
            "--staging-root" => set_once_cli_argument(
                &mut staging_root,
                next_value(&mut arguments, "--staging-root")?,
                "--staging-root",
            )?,
            "--reviewed-packet" => set_once_cli_argument(
                &mut reviewed_packet_dir,
                next_value(&mut arguments, "--reviewed-packet")?,
                "--reviewed-packet",
            )?,
            "--review-ledger" => set_once_cli_argument(
                &mut review_ledger_path,
                next_value(&mut arguments, "--review-ledger")?,
                "--review-ledger",
            )?,
            "--rule-set-id" => set_once_cli_argument(
                &mut rule_set_id,
                next_value(&mut arguments, "--rule-set-id")?,
                "--rule-set-id",
            )?,
            "--apply" if !apply => apply = true,
            "--apply" => {
                return Err(CodegenError::new("--apply may be provided only once"));
            }
            "--json" if !json_output => json_output = true,
            "--json" => {
                return Err(CodegenError::new("--json may be provided only once"));
            }
            "--help" | "-h" => {
                println!(
                    "{}",
                    [
                        "Usage: bir-rules-codegen integrate-form \\",
                        "  --staging-root EXTERNAL-DIR --reviewed-packet PACKET-DIR \\",
                        "  --review-ledger REVIEWED-LEDGER.json --rule-set-id ID \\",
                        "  [--repo-root PATH] [--apply] [--json]",
                        "",
                        "Dry-run is the default. --apply performs the checked atomic v2 source-tree transaction.",
                    ]
                    .join("\n")
                );
                return Ok(());
            }
            other => {
                return Err(CodegenError::new(format!(
                    "unknown argument `{other}` for `integrate-form`"
                )));
            }
        }
    }
    let repo_root = match repo_root {
        Some(path) => PathBuf::from(path),
        None => bir_rules_codegen::discover_default_repo_root()?,
    };
    let staging_root = staging_root
        .ok_or_else(|| CodegenError::new("`integrate-form` requires --staging-root DIR"))?;
    let reviewed_packet_dir = reviewed_packet_dir
        .ok_or_else(|| CodegenError::new("`integrate-form` requires --reviewed-packet DIR"))?;
    let review_ledger_path = review_ledger_path
        .ok_or_else(|| CodegenError::new("`integrate-form` requires --review-ledger FILE"))?;
    let rule_set_id = rule_set_id
        .ok_or_else(|| CodegenError::new("`integrate-form` requires --rule-set-id ID"))?;
    let mut options = FormIntegrationOptions::new(
        repo_root,
        staging_root,
        reviewed_packet_dir,
        review_ledger_path,
        rule_set_id,
    );
    if apply {
        options = options.with_apply();
    }
    let report = integrate_form(&options)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|source| {
                CodegenError::with_source("serialize form integration report", source)
            })?
        );
    } else {
        println!(
            "{} form `{}`: {} -> {} snapshot(s), proposed tree {}, generated output {}",
            if report.applied {
                "integrated"
            } else {
                "verified dry-run for"
            },
            report.rule_set_id,
            report.current_snapshot_count,
            report.proposed_snapshot_count,
            report.proposed_tree_sha256,
            report.generated_output_sha256,
        );
        println!(
            "changed v2 source files: {}; canonical source root: {}",
            report.changed_files.len(),
            report.canonical_source_root.display()
        );
    }
    Ok(())
}

fn run_scaffold_evidence_review_ledger(mut arguments: impl Iterator<Item = String>) -> Result<()> {
    let mut repo_root = None;
    let mut input_path = None;
    let mut vault_catalog = None;
    let mut output_path = None;
    let mut dry_run = false;
    let mut json_output = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--repo-root" => set_once_cli_argument(
                &mut repo_root,
                next_value(&mut arguments, "--repo-root")?,
                "--repo-root",
            )?,
            "--input" => set_once_cli_argument(
                &mut input_path,
                next_value(&mut arguments, "--input")?,
                "--input",
            )?,
            "--vault-catalog" => set_once_cli_argument(
                &mut vault_catalog,
                next_value(&mut arguments, "--vault-catalog")?,
                "--vault-catalog",
            )?,
            "--output" => set_once_cli_argument(
                &mut output_path,
                next_value(&mut arguments, "--output")?,
                "--output",
            )?,
            "--dry-run" if !dry_run => dry_run = true,
            "--dry-run" => {
                return Err(CodegenError::new("--dry-run may be provided only once"));
            }
            "--json" if !json_output => json_output = true,
            "--json" => {
                return Err(CodegenError::new("--json may be provided only once"));
            }
            "--help" | "-h" => {
                println!(
                    "{}",
                    [
                        "Usage: bir-rules-codegen scaffold-evidence-review-ledger \\",
                        "  --input CANONICAL-EXTERNAL-REQUEST.json \\",
                        "  --vault-catalog EXTERNAL-VAULT-CATALOG.json \\",
                        "  --output FRESH-EXTERNAL-LEDGER.json \\",
                        "  [--repo-root PATH] [--dry-run] [--json]",
                        "",
                        "The output is always candidate/null review state; this command cannot approve a packet.",
                    ]
                    .join("\n")
                );
                return Ok(());
            }
            other => {
                return Err(CodegenError::new(format!(
                    "unknown argument `{other}` for `scaffold-evidence-review-ledger`"
                )));
            }
        }
    }
    let repo_root = match repo_root {
        Some(path) => PathBuf::from(path),
        None => bir_rules_codegen::discover_default_repo_root()?,
    };
    let input_path = PathBuf::from(input_path.ok_or_else(|| {
        CodegenError::new("`scaffold-evidence-review-ledger` requires --input FILE")
    })?);
    let vault_catalog = vault_catalog.ok_or_else(|| {
        CodegenError::new("`scaffold-evidence-review-ledger` requires --vault-catalog FILE")
    })?;
    let output_path = output_path.ok_or_else(|| {
        CodegenError::new("`scaffold-evidence-review-ledger` requires --output FILE")
    })?;
    let request = load_evidence_review_scaffold_request(&repo_root, &input_path)?;
    let mut options = ScaffoldEvidenceReviewLedgerOptions::new(
        &repo_root,
        vault_catalog,
        output_path,
        request.entries,
    );
    options.dry_run = dry_run;
    let report = scaffold_evidence_review_ledger(&options)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|source| {
                CodegenError::with_source("serialize evidence review scaffold report", source)
            })?
        );
    } else {
        println!(
            "{} candidate review ledger: {} entries, ledger {}, index {}, vault catalog {}",
            if report.written {
                "wrote"
            } else {
                "verified dry-run"
            },
            report.entry_count,
            report.ledger_sha256,
            report.rules_index_sha256,
            report.vault_catalog_sha256,
        );
        println!("candidate ledger: {}", report.output_path.display());
    }
    Ok(())
}

fn run_verify_evidence_vault_source_map(mut arguments: impl Iterator<Item = String>) -> Result<()> {
    let mut repo_root = None;
    let mut source_map = None;
    let mut json_output = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--repo-root" => set_once_cli_argument(
                &mut repo_root,
                next_value(&mut arguments, "--repo-root")?,
                "--repo-root",
            )?,
            "--source-map" => set_once_cli_argument(
                &mut source_map,
                next_value(&mut arguments, "--source-map")?,
                "--source-map",
            )?,
            "--json" if !json_output => json_output = true,
            "--json" => return Err(CodegenError::new("--json may be provided only once")),
            "--help" | "-h" => {
                println!(
                    "{}",
                    [
                        "Usage: bir-rules-codegen verify-evidence-vault-source-map \\",
                        "  --source-map FILE [--repo-root PATH] [--json]",
                        "",
                        "Relative paths are resolved against the current directory before the",
                        "no-write verifier runs, so recorded argv need not contain machine paths.",
                    ]
                    .join("\n")
                );
                return Ok(());
            }
            other => {
                return Err(CodegenError::new(format!(
                    "unknown argument `{other}` for `verify-evidence-vault-source-map`"
                )));
            }
        }
    }
    let repo_root = match repo_root {
        Some(path) => PathBuf::from(path),
        None => bir_rules_codegen::discover_default_repo_root()?,
    };
    let source_map = source_map.ok_or_else(|| {
        CodegenError::new("`verify-evidence-vault-source-map` requires --source-map FILE")
    })?;
    let source_map = resolve_existing_cli_path(Path::new(&source_map), "vault source map")?;
    let report = verify_evidence_vault_source_map(&VerifyEvidenceVaultSourceMapOptions::new(
        repo_root, source_map,
    ))?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|source| {
                CodegenError::with_source("serialize vault source verification report", source)
            })?
        );
    } else {
        println!(
            "verified evidence source map: {} manifest(s), {} mapped declaration(s), {} source file(s), {} unique content file(s), map {}, verification {}",
            report.manifest_count,
            report.mapped_asset_count,
            report.verified_source_file_count,
            report.unique_content_count,
            report.source_map_sha256,
            report.verification_sha256,
        );
    }
    Ok(())
}

fn run_write_evidence_capture_metadata(mut arguments: impl Iterator<Item = String>) -> Result<()> {
    let mut repo_root = None;
    let mut output = None;
    let mut capture_session_id = None;
    let mut source_map_sha256 = None;
    let mut source_verification_sha256 = None;
    let mut tool_commit = None;
    let mut command_argv = Vec::new();
    let mut capture_tool_version = None;
    let mut operating_system = None;
    let mut windows_version = None;
    let mut official_app_version = None;
    let mut started_at_utc = None;
    let mut finished_at_utc = None;
    let mut write = false;
    let mut json_output = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--repo-root" => set_once_cli_argument(
                &mut repo_root,
                next_value(&mut arguments, "--repo-root")?,
                "--repo-root",
            )?,
            "--output" => set_once_cli_argument(
                &mut output,
                next_value(&mut arguments, "--output")?,
                "--output",
            )?,
            "--capture-session-id" => set_once_cli_argument(
                &mut capture_session_id,
                next_value(&mut arguments, "--capture-session-id")?,
                "--capture-session-id",
            )?,
            "--source-map-sha256" => set_once_cli_argument(
                &mut source_map_sha256,
                next_value(&mut arguments, "--source-map-sha256")?,
                "--source-map-sha256",
            )?,
            "--source-verification-sha256" => set_once_cli_argument(
                &mut source_verification_sha256,
                next_value(&mut arguments, "--source-verification-sha256")?,
                "--source-verification-sha256",
            )?,
            "--tool-commit" => set_once_cli_argument(
                &mut tool_commit,
                next_value(&mut arguments, "--tool-commit")?,
                "--tool-commit",
            )?,
            "--command-arg" => command_argv.push(next_value(&mut arguments, "--command-arg")?),
            "--capture-tool-version" => set_once_cli_argument(
                &mut capture_tool_version,
                next_value(&mut arguments, "--capture-tool-version")?,
                "--capture-tool-version",
            )?,
            "--operating-system" => set_once_cli_argument(
                &mut operating_system,
                next_value(&mut arguments, "--operating-system")?,
                "--operating-system",
            )?,
            "--windows-version" => set_once_cli_argument(
                &mut windows_version,
                next_value(&mut arguments, "--windows-version")?,
                "--windows-version",
            )?,
            "--official-app-version" => set_once_cli_argument(
                &mut official_app_version,
                next_value(&mut arguments, "--official-app-version")?,
                "--official-app-version",
            )?,
            "--started-at-utc" => set_once_cli_argument(
                &mut started_at_utc,
                next_value(&mut arguments, "--started-at-utc")?,
                "--started-at-utc",
            )?,
            "--finished-at-utc" => set_once_cli_argument(
                &mut finished_at_utc,
                next_value(&mut arguments, "--finished-at-utc")?,
                "--finished-at-utc",
            )?,
            "--write" if !write => write = true,
            "--write" => return Err(CodegenError::new("--write may be provided only once")),
            "--json" if !json_output => json_output = true,
            "--json" => return Err(CodegenError::new("--json may be provided only once")),
            "--help" | "-h" => {
                println!(
                    "{}",
                    [
                        "Usage: bir-rules-codegen write-evidence-capture-metadata \\",
                        "  --output FRESH-EXTERNAL-FILE --capture-session-id ID \\",
                        "  --source-map-sha256 SHA256 --source-verification-sha256 SHA256 \\",
                        "  --tool-commit FULL-SHA --command-arg ARG [--command-arg ARG ...] \\",
                        "  --capture-tool-version TEXT --operating-system windows \\",
                        "  --windows-version TEXT --official-app-version TEXT \\",
                        "  --started-at-utc TIMESTAMP --finished-at-utc TIMESTAMP \\",
                        "  [--repo-root PATH] [--write] [--json]",
                        "",
                        "Dry-run is the default. --write publishes one fresh canonical file.",
                    ]
                    .join("\n")
                );
                return Ok(());
            }
            other => {
                return Err(CodegenError::new(format!(
                    "unknown argument `{other}` for `write-evidence-capture-metadata`"
                )));
            }
        }
    }
    let repo_root = match repo_root {
        Some(path) => PathBuf::from(path),
        None => bir_rules_codegen::discover_default_repo_root()?,
    };
    let output = output.ok_or_else(|| {
        CodegenError::new("`write-evidence-capture-metadata` requires --output FILE")
    })?;
    let operating_system = operating_system.ok_or_else(|| {
        CodegenError::new("`write-evidence-capture-metadata` requires --operating-system windows")
    })?;
    if operating_system != "windows" {
        return Err(CodegenError::new(
            "`write-evidence-capture-metadata` supports only --operating-system windows",
        ));
    }
    let capture_provenance = EvidenceCaptureProvenance {
        tool_commit: required_cli_value(
            tool_commit,
            "write-evidence-capture-metadata",
            "--tool-commit",
        )?,
        command_argv,
        capture_tool_version: required_cli_value(
            capture_tool_version,
            "write-evidence-capture-metadata",
            "--capture-tool-version",
        )?,
        operating_system: EvidenceCaptureOperatingSystem::Windows,
        windows_version: required_cli_value(
            windows_version,
            "write-evidence-capture-metadata",
            "--windows-version",
        )?,
        official_app_version: required_cli_value(
            official_app_version,
            "write-evidence-capture-metadata",
            "--official-app-version",
        )?,
        started_at_utc: required_cli_value(
            started_at_utc,
            "write-evidence-capture-metadata",
            "--started-at-utc",
        )?,
        finished_at_utc: required_cli_value(
            finished_at_utc,
            "write-evidence-capture-metadata",
            "--finished-at-utc",
        )?,
    };
    let output = resolve_fresh_cli_path(Path::new(&output), "capture-metadata output")?;
    let mut options = WriteEvidenceVaultCaptureMetadataOptions::new(
        repo_root,
        output,
        required_cli_value(
            capture_session_id,
            "write-evidence-capture-metadata",
            "--capture-session-id",
        )?,
        required_cli_value(
            source_map_sha256,
            "write-evidence-capture-metadata",
            "--source-map-sha256",
        )?,
        required_cli_value(
            source_verification_sha256,
            "write-evidence-capture-metadata",
            "--source-verification-sha256",
        )?,
        capture_provenance,
    );
    options.dry_run = !write;
    let report = write_evidence_vault_capture_metadata(&options)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|source| {
                CodegenError::with_source("serialize capture-metadata write report", source)
            })?
        );
    } else {
        println!(
            "{} capture metadata {} at {}",
            if report.written {
                "wrote"
            } else {
                "verified dry-run"
            },
            report.capture_metadata_sha256,
            report.output_path.display()
        );
    }
    Ok(())
}

fn run_acquire_evidence_vault(mut arguments: impl Iterator<Item = String>) -> Result<()> {
    let mut repo_root = None;
    let mut source_map = None;
    let mut capture_metadata = None;
    let mut vault_root = None;
    let mut dry_run = false;
    let mut json_output = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--repo-root" => set_once_cli_argument(
                &mut repo_root,
                next_value(&mut arguments, "--repo-root")?,
                "--repo-root",
            )?,
            "--source-map" => set_once_cli_argument(
                &mut source_map,
                next_value(&mut arguments, "--source-map")?,
                "--source-map",
            )?,
            "--capture-metadata" => set_once_cli_argument(
                &mut capture_metadata,
                next_value(&mut arguments, "--capture-metadata")?,
                "--capture-metadata",
            )?,
            "--vault-root" => set_once_cli_argument(
                &mut vault_root,
                next_value(&mut arguments, "--vault-root")?,
                "--vault-root",
            )?,
            "--dry-run" => dry_run = true,
            "--json" => json_output = true,
            "--help" | "-h" => {
                println!(
                    "{}",
                    [
                        "Usage: bir-rules-codegen acquire-evidence-vault \\",
                        "  --source-map FILE --capture-metadata FILE --vault-root FRESH-DIR \\",
                        "  [--repo-root PATH] [--dry-run] [--json]",
                    ]
                    .join("\n")
                );
                return Ok(());
            }
            other => {
                return Err(CodegenError::new(format!(
                    "unknown argument `{other}` for `acquire-evidence-vault`"
                )));
            }
        }
    }
    let repo_root = match repo_root {
        Some(path) => PathBuf::from(path),
        None => bir_rules_codegen::discover_default_repo_root()?,
    };
    let source_map = source_map
        .ok_or_else(|| CodegenError::new("`acquire-evidence-vault` requires --source-map FILE"))?;
    let capture_metadata = capture_metadata.ok_or_else(|| {
        CodegenError::new("`acquire-evidence-vault` requires --capture-metadata FILE")
    })?;
    let vault_root = vault_root.ok_or_else(|| {
        CodegenError::new("`acquire-evidence-vault` requires --vault-root FRESH-DIR")
    })?;
    let mut options =
        AcquireEvidenceVaultOptions::new(repo_root, source_map, capture_metadata, vault_root);
    options.dry_run = dry_run;
    let report = acquire_evidence_vault(&options)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|source| {
                CodegenError::with_source("serialize evidence vault acquisition report", source)
            })?
        );
    } else {
        println!(
            "{} evidence vault: {} manifest(s), {} declared asset(s), {} mapped asset(s), {} unique content file(s), {} metadata-only gap(s), source map {}, verification {}, catalog {}",
            if report.written {
                "acquired"
            } else {
                "verified dry-run"
            },
            report.manifest_count,
            report.declared_asset_count,
            report.mapped_asset_count,
            report.unique_content_count,
            report.gaps.len(),
            report.source_map_sha256,
            report.source_verification_sha256,
            report.catalog_sha256,
        );
        println!("vault root: {}", report.vault_root.display());
        println!("catalog: {}", report.catalog_path.display());
    }
    Ok(())
}

fn resolve_existing_cli_path(path: &Path, label: &str) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|source| CodegenError::with_source("resolve current directory", source))?
            .join(path)
    };
    fs::canonicalize(&candidate)
        .map_err(|source| CodegenError::io(&format!("resolve {label}"), &candidate, source))
}

fn resolve_fresh_cli_path(path: &Path, label: &str) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|source| CodegenError::with_source("resolve current directory", source))?
            .join(path)
    };
    let parent = candidate.parent().ok_or_else(|| {
        CodegenError::new(format!("{label} `{}` has no parent", candidate.display()))
    })?;
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|source| CodegenError::io(&format!("resolve {label} parent"), parent, source))?;
    let file_name = candidate.file_name().ok_or_else(|| {
        CodegenError::new(format!(
            "{label} `{}` has no final file name",
            candidate.display()
        ))
    })?;
    Ok(canonical_parent.join(file_name))
}

fn required_cli_value(value: Option<String>, command: &str, flag: &str) -> Result<String> {
    value.ok_or_else(|| CodegenError::new(format!("`{command}` requires {flag} VALUE")))
}

fn set_once_cli_argument(slot: &mut Option<String>, value: String, flag: &str) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(CodegenError::new(format!(
            "{flag} may be provided only once"
        )));
    }
    Ok(())
}

fn usage_error() -> CodegenError {
    CodegenError::new(usage())
}

fn print_usage() {
    println!("{}", usage());
}

fn usage() -> String {
    "Usage: bir-rules-codegen <audit|generate|check|validate-v1|status|coverage|operator-census|reconciliation|build-2550q-bindings|project-2550q|roll-pin|discover-evidence-vault-sources|verify-evidence-vault-source-map|write-evidence-capture-metadata|acquire-evidence-vault|scaffold-evidence-review-ledger|verify-evidence|import-evidence|stage-form|stage-evidence-packet-review|build-evidence-packet|build-evidence-packet-set|check-evidence-packet-set|integrate-form> [options]\n\
     Options:\n\
     \x20 --repo-root PATH\n\
     \x20 --source-dir REPOSITORY/RELATIVE/PATH\n\
     \x20 --schema-dir REPOSITORY/RELATIVE/PATH\n\
     \x20 --output-dir REPOSITORY/RELATIVE/PATH\n\
     \x20 --rules-dir REPOSITORY/RELATIVE/PATH   (validate-v1 only)\n\
     \x20 --output REPOSITORY/RELATIVE/PATH      (build-2550q-bindings only)\n\
     \x20 --json                 (validate-v1, status, coverage, operator-census, reconciliation)\n\
     \x20 --require-promotion    (status only; require deferred promotion criteria)\n\
     \x20 --boundaries-only      (status only; check production boundaries only)\n\
     \x20 --rule-set-id ID       (audit, generate, check, or roll-pin)\n\
     \x20 --all                  (roll-pin only; all snapshots, verification-only)\n\
     \x20 --dry-run              (roll-pin only)\n\
     \x20 --staging-root REPOSITORY/RELATIVE/PATH   (project-2550q only)\n\
     \x20 --skip-runtime-tests   (check only)\n\
     Run an evidence command with --help for its command-specific options."
        .to_owned()
}

#[cfg(test)]
mod tests {
    #[test]
    fn vault_acquisition_rejects_duplicate_external_path_flags() {
        let error = super::run_acquire_evidence_vault(
            ["--source-map", "first.json", "--source-map", "second.json"]
                .into_iter()
                .map(str::to_owned),
        )
        .expect_err("duplicate source map must fail closed");
        assert_eq!(error.message(), "--source-map may be provided only once");
    }

    #[test]
    fn usage_documents_explicit_promotion_mode() {
        let usage = super::usage();
        assert!(usage.contains("--require-promotion"));
        assert!(usage.contains("status only; require deferred promotion criteria"));
        assert!(usage.contains("--boundaries-only"));
        assert!(usage.contains("status only; check production boundaries only"));
        assert!(usage.contains("audit, generate, check, or roll-pin"));
    }
}
