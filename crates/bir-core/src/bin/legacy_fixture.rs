//! `legacy-fixture <run-folder>/bir_data.db` — the BIR_DEVICE_DIR contract's generator: an
//! unlocked pre-rooms database (the current schema, as v0.2.0 writes it) with one taxpayer
//! profile and one Queued return. The run folder is the target's parent. Debug builds in the
//! test environment only; anything else refuses. test-support only (required-features), so no
//! normal build contains it.

fn main() -> std::process::ExitCode {
    if !(cfg!(debug_assertions) && bir_core::db::test_environment()) {
        eprintln!("legacy-fixture is a debug-build, test-environment tool only");
        return std::process::ExitCode::from(2);
    }
    let mut args = std::env::args().skip(1);
    let Some(target) = args.next() else {
        eprintln!("usage: legacy-fixture <run-folder>/bir_data.db");
        return std::process::ExitCode::from(2);
    };
    let target = std::path::PathBuf::from(target);
    let Some(run_folder) = target.parent().map(std::path::Path::to_path_buf) else {
        eprintln!("legacy-fixture: the target has no parent folder");
        return std::process::ExitCode::from(2);
    };
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
