//! macOS-specific implementations for bir-core platform services.

use std::path::PathBuf;
use tracing::{info, warn};

// ── Data Directory ───────────────────────────────────────────────────────────

/// Returns the macOS application data directory.
///
/// Uses the App Group container so the database can be shared between the main
/// app and the background daemon, even under App Sandbox.
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

// ── Daemon Installer ─────────────────────────────────────────────────────────

#[cfg(not(feature = "mas_build"))]
pub fn install_daemon() {
    if let Ok(exe_path) = std::env::current_exe() {
        let daemon_path = exe_path.parent().unwrap().join("bir-daemon");
        let plist_content = generate_plist(&daemon_path);

        let home_dir = std::env::var("HOME").expect("HOME not set");
        let launch_agents_dir = std::path::Path::new(&home_dir)
            .join("Library")
            .join("LaunchAgents");
        let _ = std::fs::create_dir_all(&launch_agents_dir);
        let plist_path = launch_agents_dir.join("com.bir.vault.daemon.plist");

        if let Err(e) = std::fs::write(&plist_path, plist_content) {
            warn!("Failed to write LaunchAgent plist: {}", e);
            return;
        }

        // Unload existing to refresh
        let _ = std::process::Command::new("launchctl")
            .arg("unload")
            .arg(&plist_path)
            .output();

        // Load new
        match std::process::Command::new("launchctl")
            .arg("load")
            .arg(&plist_path)
            .output()
        {
            Ok(out) if out.status.success() => info!("macOS LaunchAgent loaded successfully"),
            _ => warn!("Failed to load macOS LaunchAgent"),
        }
    }
}

#[cfg(feature = "mas_build")]
pub fn install_daemon() {
    info!("App Store build: Skipping LaunchAgent installation. Background tasks run in-app.");
}

#[cfg(not(feature = "mas_build"))]
pub fn uninstall_daemon() {
    let home_dir = std::env::var("HOME").unwrap_or_default();
    let plist_path = std::path::Path::new(&home_dir)
        .join("Library")
        .join("LaunchAgents")
        .join("com.bir.vault.daemon.plist");

    let _ = std::process::Command::new("launchctl")
        .arg("unload")
        .arg(&plist_path)
        .output();

    let _ = std::fs::remove_file(plist_path);
    info!("macOS LaunchAgent uninstalled");
}

#[cfg(feature = "mas_build")]
pub fn uninstall_daemon() {
    info!("App Store build: Skipping LaunchAgent uninstallation.");
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

pub(crate) fn generate_plist(daemon_path: &std::path::Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.bir.vault.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>"#,
        daemon_path.display()
    )
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

    #[test]
    fn test_plist_generation() {
        let dummy_path =
            std::path::PathBuf::from("/Applications/Test.app/Contents/MacOS/bir-daemon");
        let plist = generate_plist(&dummy_path);

        assert!(plist.contains("<string>com.bir.vault.daemon</string>"));
        assert!(
            plist.contains("<string>/Applications/Test.app/Contents/MacOS/bir-daemon</string>")
        );
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains("<key>KeepAlive</key>"));
    }
}
