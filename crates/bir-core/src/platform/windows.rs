//! Windows-specific implementations for bir-core platform services.

use std::path::PathBuf;

/// Prevents console windows from flashing on screen when spawning subprocesses.
const CREATE_NO_WINDOW: u32 = 0x08000000;

// ── Data Directory ───────────────────────────────────────────────────────────

/// Returns the Windows application data directory.
///
/// Uses `%LOCALAPPDATA%\Taxman\eBIRForms` following Windows conventions.
#[cfg(feature = "sqlcipher")]
pub(super) fn data_dir_path() -> Option<PathBuf> {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return Some(
            PathBuf::from(local_app_data)
                .join("Taxman")
                .join("eBIRForms"),
        );
    }

    // Fallback
    std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".taxman-ebir"))
}

/// Returns the Windows application data directory of the plain-SQLite (NSH)
/// build: `%LOCALAPPDATA%\dev.goldcoders.bir`.
#[cfg(not(feature = "sqlcipher"))]
pub(super) fn data_dir_path() -> Option<PathBuf> {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return Some(PathBuf::from(local_app_data).join(super::NSH_APP_ID));
    }

    // Fallback
    std::env::var_os("USERPROFILE").map(|home| {
        PathBuf::from(home)
            .join("AppData")
            .join("Local")
            .join(super::NSH_APP_ID)
    })
}

/// [`data_dir_path`], created if missing. Without either folder: `.taxman-ebir`, not created.
pub fn data_dir() -> PathBuf {
    super::create_data_dir(data_dir_path())
}

/// Returns the temporary directory.
pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

// ── Shell Execution ──────────────────────────────────────────────────────────

/// Execute a shell command using the platform's native shell.
pub async fn run_shell_command(cmd: &str) -> Result<std::process::Output, std::io::Error> {
    tokio::process::Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .arg("/c")
        .arg(cmd)
        .output()
        .await
}
