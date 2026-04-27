//! Windows-specific implementations for bir-core platform services.

use std::path::PathBuf;
use tracing::{info, warn};

// ── Data Directory ───────────────────────────────────────────────────────────

/// Returns the Windows application data directory.
///
/// Uses `%LOCALAPPDATA%\Taxman\eBIRForms` following Windows conventions.
pub fn data_dir() -> PathBuf {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let dir = PathBuf::from(local_app_data)
            .join("Taxman")
            .join("eBIRForms");
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }

    // Fallback
    if let Some(home) = std::env::var_os("USERPROFILE") {
        let dir = PathBuf::from(home).join(".taxman-ebir");
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }

    PathBuf::from(".taxman-ebir")
}

/// Returns the temporary directory.
pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

// ── Daemon Installer ─────────────────────────────────────────────────────────

pub fn install_daemon() {
    if let Ok(exe_path) = std::env::current_exe() {
        let daemon_path = exe_path.parent().unwrap().join("bir-daemon.exe");
        let daemon_path_str = daemon_path.to_string_lossy().to_string();

        let _ = std::process::Command::new("reg")
            .args(&[
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "BIRVaultDaemon",
                "/t",
                "REG_SZ",
                "/d",
                &daemon_path_str,
                "/f",
            ])
            .output();

        // Start it immediately
        let _ = std::process::Command::new(&daemon_path).spawn();
        info!("Windows Registry Run key added");
    }
}

pub fn uninstall_daemon() {
    let _ = std::process::Command::new("reg")
        .args(&[
            "delete",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            "/v",
            "BIRVaultDaemon",
            "/f",
        ])
        .output();

    // Kill the process if running
    let _ = std::process::Command::new("taskkill")
        .args(&["/F", "/IM", "bir-daemon.exe"])
        .output();
    info!("Windows Registry Run key removed");
}

// ── Shell Execution ──────────────────────────────────────────────────────────

/// Execute a shell command using the platform's native shell.
pub async fn run_shell_command(cmd: &str) -> Result<std::process::Output, std::io::Error> {
    tokio::process::Command::new("cmd")
        .arg("/c")
        .arg(cmd)
        .output()
        .await
}
