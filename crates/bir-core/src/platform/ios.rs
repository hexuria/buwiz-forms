//! iOS implementations for bir-core platform services.

use std::path::PathBuf;

/// Returns the app's data directory inside its sandbox:
/// `$HOME/Library/Application Support/dev.goldcoders.bir`.
pub fn data_dir() -> PathBuf {
    let dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Library")
        .join("Application Support")
        .join(super::NSH_APP_ID);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Returns the temporary directory.
pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

/// iOS apps cannot spawn processes, so scheduled shell commands always fail.
pub async fn run_shell_command(_cmd: &str) -> Result<std::process::Output, std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "shell commands are not available on iOS",
    ))
}

/// See `super::prepare_app_storage`. The marker sits in [`data_dir`], which
/// goes when the app is deleted.
pub(super) fn prepare_ios_storage() {
    use bir_submit::ios_storage::{
        INSTALL_MARKER, OsKeychain, clear_on_fresh_install, exclude_from_backup,
    };
    let dir = data_dir();
    if let Err(error) = exclude_from_backup(&dir) {
        tracing::warn!("the app's data folder could not be excluded from backups: {error}");
    }
    let outcome = clear_on_fresh_install(
        &OsKeychain,
        !super::keychain_disabled(),
        &dir.join(INSTALL_MARKER),
    );
    tracing::info!("Keychain at launch: {outcome:?}");
}
