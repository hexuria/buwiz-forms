//! Linux-specific implementations for bir-core platform services.

use std::path::PathBuf;
use tracing::{info, warn};

// ── Data Directory ───────────────────────────────────────────────────────────

/// Returns the Linux application data directory.
///
/// Uses `$HOME/.taxman-ebir` following XDG-compatible conventions.
pub fn data_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
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
        let daemon_path = exe_path.parent().unwrap().join("bir-daemon");

        let service_content = format!(
            r#"[Unit]
Description=BIR Vault Background Daemon
After=network.target

[Service]
Type=simple
ExecStart={}
Restart=always
RestartSec=10

[Install]
WantedBy=default.target"#,
            daemon_path.display()
        );

        let home_dir = std::env::var("HOME").expect("HOME not set");
        let systemd_dir = std::path::Path::new(&home_dir)
            .join(".config")
            .join("systemd")
            .join("user");
        let _ = std::fs::create_dir_all(&systemd_dir);
        let service_path = systemd_dir.join("bir-vault-daemon.service");

        if let Err(e) = std::fs::write(&service_path, service_content) {
            warn!("Failed to write systemd service: {}", e);
            return;
        }

        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "enable", "--now", "bir-vault-daemon.service"])
            .output();
        info!("Linux systemd service installed");
    }
}

pub fn uninstall_daemon() {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "disable", "--now", "bir-vault-daemon.service"])
        .output();

    let home_dir = std::env::var("HOME").unwrap_or_default();
    let service_path = std::path::Path::new(&home_dir)
        .join(".config")
        .join("systemd")
        .join("user")
        .join("bir-vault-daemon.service");
    let _ = std::fs::remove_file(service_path);

    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();
    info!("Linux systemd service uninstalled");
}

pub fn is_daemon_running() -> bool {
    std::process::Command::new("systemctl")
        .args(["--user", "is-active", "bir-vault-daemon.service"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
