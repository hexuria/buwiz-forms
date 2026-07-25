use std::env;
use std::error::Error as _;
use std::path::PathBuf;
use std::process::ExitCode;

use bir_rules_codegen::{
    AuditOptions, CheckOptions, CodegenError, GenerateOptions, Result, audit, check, generate,
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
    if !matches!(command.as_str(), "audit" | "generate" | "check") {
        return Err(CodegenError::new(format!(
            "unknown command `{command}`\n\n{}",
            usage()
        )));
    }

    let mut repo_root = None;
    let mut source_dir = None;
    let mut schema_dir = None;
    let mut output_dir = None;
    let mut skip_runtime_tests = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--repo-root" => repo_root = Some(next_value(&mut arguments, "--repo-root")?),
            "--source-dir" => source_dir = Some(next_value(&mut arguments, "--source-dir")?),
            "--schema-dir" => schema_dir = Some(next_value(&mut arguments, "--schema-dir")?),
            "--output-dir" => output_dir = Some(next_value(&mut arguments, "--output-dir")?),
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
    let mut audit_options = AuditOptions::new(&repo_root);
    if let Some(source_dir) = source_dir {
        audit_options.source_dir = source_dir;
    }
    if let Some(schema_dir) = schema_dir {
        audit_options.schema_dir = schema_dir;
    }

    match command.as_str() {
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
    "Usage: bir-rules-codegen <audit|generate|check> [options]\n\
     Options:\n\
     \x20 --repo-root PATH\n\
     \x20 --source-dir REPOSITORY/RELATIVE/PATH\n\
     \x20 --schema-dir REPOSITORY/RELATIVE/PATH\n\
     \x20 --output-dir REPOSITORY/RELATIVE/PATH\n\
     \x20 --skip-runtime-tests   (check only)"
        .to_owned()
}
