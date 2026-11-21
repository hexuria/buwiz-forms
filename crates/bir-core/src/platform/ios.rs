//! iOS implementations for bir-core platform services.

use std::path::PathBuf;

/// Returns the app's data directory inside its sandbox:
/// `$HOME/Library/Application Support/dev.goldcoders.bir.nsh`.
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
