//! `legacy-fixture <run-folder> <run-folder>/bir_data.db` — the BIR_DEVICE_DIR contract's
//! generator: an unlocked pre-rooms database (the current schema, as v0.2.0 writes it) with
//! one taxpayer profile and one Queued return. Debug builds in the test environment only;
//! anything else refuses.

#[cfg(feature = "test-support")]
fn main() -> std::process::ExitCode {
    if !(cfg!(debug_assertions) && bir_core::db::test_environment()) {
        eprintln!("legacy-fixture is a debug-build, test-environment tool only");
        return std::process::ExitCode::from(2);
    }
    let mut args = std::env::args().skip(1);
    let (Some(run_folder), Some(target)) = (args.next(), args.next()) else {
        eprintln!("usage: legacy-fixture <run-folder> <run-folder>/bir_data.db");
        return std::process::ExitCode::from(2);
    };
    let run_folder = std::path::PathBuf::from(run_folder);
    let target = std::path::PathBuf::from(target);
    match bir_core::legacy_claim::fixture::write_legacy_fixture(&run_folder, &target) {
        Ok(()) => {
            println!("legacy fixture written at {}", target.display());
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(not(feature = "test-support"))]
fn main() -> std::process::ExitCode {
    eprintln!("legacy-fixture needs bir-core's test-support feature (`just legacy-fixture`)");
    std::process::ExitCode::from(2)
}
