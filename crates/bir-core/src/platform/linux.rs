//! Linux-specific implementations for bir-core platform services.

use std::path::PathBuf;

// ── Data Directory ───────────────────────────────────────────────────────────

/// Returns the Linux application data directory.
///
/// Uses `$HOME/.taxman-ebir` following XDG-compatible conventions.
#[cfg(feature = "sqlcipher")]
pub fn data_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let dir = PathBuf::from(home).join(".taxman-ebir");
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }

    PathBuf::from(".taxman-ebir")
}

/// Returns the Linux application data directory of the plain-SQLite (NSH)
/// build: `$XDG_DATA_HOME/dev.goldcoders.bir.nsh`, or
/// `$HOME/.local/share/dev.goldcoders.bir.nsh` when `XDG_DATA_HOME` is not an
/// absolute path.
#[cfg(not(feature = "sqlcipher"))]
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        });
    if let Some(base) = base {
        let dir = base.join(super::NSH_APP_ID);
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }

    PathBuf::from(".taxman-ebir")
}

/// Returns the temporary directory.
pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

// ── Shell Execution ──────────────────────────────────────────────────────────

/// Execute a shell command using the platform's native shell.
pub async fn run_shell_command(cmd: &str) -> Result<std::process::Output, std::io::Error> {
    tokio::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .await
}
