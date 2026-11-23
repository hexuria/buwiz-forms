//! macOS-specific implementations for bir-core platform services.
#![allow(unexpected_cfgs)]

use std::path::PathBuf;

// ── Data Directory ───────────────────────────────────────────────────────────

/// Returns the macOS application data directory.
///
/// Uses the App Group container so the database can be shared between the main
/// app and background tasks, even under App Sandbox.
#[cfg(feature = "sqlcipher")]
pub fn data_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let group_container = PathBuf::from(home)
            .join("Library")
            .join("Group Containers")
            .join("group.dev.goldcoders.bir");

        let _ = std::fs::create_dir_all(&group_container);
        return group_container;
    }

    // Fallback (should never happen on macOS)
    PathBuf::from(".taxman-ebir")
}

/// Returns the macOS application data directory of the plain-SQLite (NSH)
/// build: `~/Library/Application Support/dev.goldcoders.bir`. Never the
/// group container of the old app.
#[cfg(not(feature = "sqlcipher"))]
pub fn data_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let dir = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join(super::NSH_APP_ID);
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }

    // Fallback (should never happen on macOS)
    PathBuf::from(".taxman-ebir")
}

/// Returns the temporary directory.
pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

// ── Shell Execution ──────────────────────────────────────────────────────────

/// Execute a shell command using the platform's native shell.
///
/// Absent from the Mac App Store build (`mas_build`): App Review rejects
/// arbitrary code execution and the sandbox would block it anyway.
#[cfg(not(feature = "mas_build"))]
pub async fn run_shell_command(cmd: &str) -> Result<std::process::Output, std::io::Error> {
    tokio::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "sqlcipher")]
    #[test]
    fn test_data_dir_resolution() {
        // Use temp_env to safely set HOME without unsafe — also auto-restores on panic.
        temp_env::with_var("HOME", Some("/Users/testuser"), || {
            let path = data_dir();
            assert!(path.to_string_lossy().contains("group.dev.goldcoders.bir"));
            assert!(
                path.to_string_lossy()
                    .contains("/Users/testuser/Library/Group Containers")
            );
        });
    }

    #[cfg(not(feature = "sqlcipher"))]
    #[test]
    fn the_plain_build_keeps_its_data_in_the_nsh_app_folder() {
        let home = tempfile::tempdir().expect("temporary home");
        temp_env::with_var("HOME", Some(home.path()), || {
            let path = data_dir();
            assert_eq!(
                path,
                home.path()
                    .join("Library")
                    .join("Application Support")
                    .join("dev.goldcoders.bir")
            );
            assert!(path.is_dir());
            assert!(!path.to_string_lossy().contains("Group Containers"));
        });
    }
}
