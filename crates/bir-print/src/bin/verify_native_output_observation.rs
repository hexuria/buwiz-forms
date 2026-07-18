//! Validate development-only native-output observations emitted by the app.
//!
//! This command deliberately cannot create release evidence or change a form
//! capability. It only applies the Rust-owned non-promotional observation
//! schema and reports the collector gaps that still block promotion.

use bir_print::html_output_evidence::{
    decode_development_native_output_observation, DevelopmentEvidenceAvailability,
};
use std::{env, fs, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let paths = env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if paths.is_empty() {
        eprintln!(
            "usage: verify_native_output_observation <observation.json> [observation.json ...]"
        );
        return ExitCode::from(2);
    }

    let mut failed = false;
    for path in paths {
        let result = fs::read(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))
            .and_then(|bytes| {
                decode_development_native_output_observation(&bytes)
                    .map_err(|error| format!("{} is invalid: {error}", path.display()))
            });

        match result {
            Ok(observation) => {
                let page_count = observation.geometry_reports[0].page_count;
                let source = match &observation.source_revision {
                    DevelopmentEvidenceAvailability::Observed { value } => value.as_str(),
                    DevelopmentEvidenceAvailability::Unavailable { .. } => "unavailable",
                };
                println!(
                    "validated development diagnostic: {} {}:{} pages={} source={} promotion_eligible=false",
                    path.display(),
                    observation.form_code,
                    observation.form_revision,
                    page_count,
                    source,
                );
                for gap in observation.strict_verifier_gaps {
                    println!("  blocking gap: {gap}");
                }
            }
            Err(error) => {
                eprintln!("{error}");
                failed = true;
            }
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
