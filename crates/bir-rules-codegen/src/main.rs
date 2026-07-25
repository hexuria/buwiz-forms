use std::env;
use std::error::Error as _;
use std::path::PathBuf;
use std::process::ExitCode;

use bir_rules_codegen::{
    AuditOptions, BuildBindingsOptions, CheckOptions, CodegenError, CoverageOptions,
    GenerateOptions, ProjectStaticSurfaceOptions, Result, RollPinOptions, StatusOptions,
    ValidateV1Options, audit, build_2550q_bindings, check, coverage, generate,
    project_2550q_static_surface, roll_pin, status, validate_v1,
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
    let mut dry_run = false;
    let mut staging_root = None;
    let mut json_output = false;
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
            "--rule-set-id" if command == "roll-pin" => {
                rule_set_id = Some(next_value(&mut arguments, "--rule-set-id")?);
            }
            "--dry-run" if command == "roll-pin" => dry_run = true,
            "--staging-root" if command == "project-2550q" => {
                staging_root = Some(next_value(&mut arguments, "--staging-root")?);
            }
            "--json" if matches!(command.as_str(), "validate-v1" | "status" | "coverage") => {
                json_output = true;
            }
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
                    "{:<16}{:>7}{:>7}{:>7}   {:<11}{:>5}{:>6}",
                    "form", "fields", "rules", "calcs", "v2", "v2r", "v2c"
                );
                for form in &report.forms {
                    println!(
                        "{:<16}{:>7}{:>7}{:>7}   {:<11}{:>5}{:>6}",
                        form.form_id,
                        form.v1_fields,
                        form.v1_rules,
                        form.v1_calculations,
                        form.v2_review_status.as_deref().unwrap_or("-"),
                        form.v2_rules,
                        form.v2_calculations,
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
                    report.v2_rules,
                    report.v1_rules,
                    report.rule_coverage_percent()
                );
                println!(
                    "executable calculations {}/{} ({:.1}%)",
                    report.v2_calculations,
                    report.v1_calculations,
                    report.calculation_coverage_percent()
                );
            }
        }
        "status" => {
            if v2_options_present {
                return Err(CodegenError::new(
                    "`status` reads the tracked corpus and does not accept source/schema/output options",
                ));
            }
            let report = status(&StatusOptions::new(&repo_root))?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|source| {
                        CodegenError::with_source("serialize status report", source)
                    })?
                );
            } else {
                println!("2550Q slice status ({})", report.rule_set_id);
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
                    "a production boundary is no longer closed; this is not an unfinished slice",
                ));
            }
            if !report.complete() {
                let open: Vec<&str> = report.open().map(|criterion| criterion.id).collect();
                return Err(CodegenError::new(format!(
                    "slice incomplete; open criteria: {}",
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
            let rule_set_id = rule_set_id
                .ok_or_else(|| CodegenError::new("`roll-pin` requires --rule-set-id <id>"))?;
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
            println!(
                "v2 audit passed: {} snapshot(s), schema {}, source {}",
                report.snapshot_count(),
                report.schema_digest(),
                report.normalized_source_digest()
            );
        }
        "generate" => {
            if skip_runtime_tests {
                return Err(CodegenError::new(
                    "`generate` does not accept --skip-runtime-tests",
                ));
            }
            let mut options = GenerateOptions::new(&repo_root);
            options.audit = audit_options;
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
        }
        "check" => {
            let mut options = CheckOptions::new(&repo_root);
            options.generate.audit = audit_options;
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

fn usage_error() -> CodegenError {
    CodegenError::new(usage())
}

fn print_usage() {
    println!("{}", usage());
}

fn usage() -> String {
    "Usage: bir-rules-codegen <audit|generate|check|validate-v1|status|build-2550q-bindings|project-2550q> [options]\n\
     Options:\n\
     \x20 --repo-root PATH\n\
     \x20 --source-dir REPOSITORY/RELATIVE/PATH\n\
     \x20 --schema-dir REPOSITORY/RELATIVE/PATH\n\
     \x20 --output-dir REPOSITORY/RELATIVE/PATH\n\
     \x20 --rules-dir REPOSITORY/RELATIVE/PATH   (validate-v1 only)\n\
     \x20 --output REPOSITORY/RELATIVE/PATH      (build-2550q-bindings only)\n\
     \x20 --json                 (validate-v1, status)\n\
     \x20 --output REPOSITORY/RELATIVE/PATH   (build-2550q-bindings only)\n\
     \x20 --rule-set-id ID       (roll-pin only)\n\
     \x20 --dry-run              (roll-pin only)\n\
     \x20 --staging-root REPOSITORY/RELATIVE/PATH   (project-2550q only)\n\
     \x20 --skip-runtime-tests   (check only)"
        .to_owned()
}
