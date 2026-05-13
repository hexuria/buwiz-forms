//! macOS-specific implementations for bir-core platform services.
#![allow(unexpected_cfgs)]

use std::path::PathBuf;

// ── Data Directory ───────────────────────────────────────────────────────────

/// Returns the macOS application data directory.
///
/// Uses the App Group container so the database can be shared between the main
/// app and background tasks, even under App Sandbox.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_dir_resolution() {
        // Temporarily set HOME to ensure consistent test output
        let old_home = std::env::var_os("HOME");
        unsafe {
            std::env::set_var("HOME", "/Users/testuser");
        }

        let path = data_dir();
        assert!(path.to_string_lossy().contains("group.dev.goldcoders.bir"));
        assert!(
            path.to_string_lossy()
                .contains("/Users/testuser/Library/Group Containers")
        );

        // Restore
        unsafe {
            if let Some(h) = old_home {
                std::env::set_var("HOME", h);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }
}
