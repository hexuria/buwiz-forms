//! Cross-platform service installer for the background daemon.

use std::env;
use tracing::{info, warn};

/// Install the background service to run automatically on user login.
pub fn install() {
    install_impl();
}

/// Uninstall the background service.
pub fn uninstall() {
    uninstall_impl();
}

#[cfg(target_os = "macos")]
fn install_impl() {
    if let Ok(exe_path) = env::current_exe() {
        // Resolve the daemon path. If we are running as `bir`, the daemon is in the same directory.
        let daemon_path = exe_path.parent().unwrap().join("bir-daemon");
        
        let plist_content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
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
</plist>"#, daemon_path.display());

        let home_dir = std::env::var("HOME").expect("HOME not set");
        let launch_agents_dir = std::path::Path::new(&home_dir).join("Library").join("LaunchAgents");
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

#[cfg(target_os = "macos")]
fn uninstall_impl() {
    let home_dir = std::env::var("HOME").unwrap_or_default();
    let plist_path = std::path::Path::new(&home_dir).join("Library").join("LaunchAgents").join("com.bir.vault.daemon.plist");
    
    let _ = std::process::Command::new("launchctl")
        .arg("unload")
        .arg(&plist_path)
        .output();
        
    let _ = std::fs::remove_file(plist_path);
    info!("macOS LaunchAgent uninstalled");
}

#[cfg(target_os = "windows")]
fn install_impl() {
    if let Ok(exe_path) = env::current_exe() {
        let daemon_path = exe_path.parent().unwrap().join("bir-daemon.exe");
        let daemon_path_str = daemon_path.to_string_lossy().to_string();
        
        let _ = std::process::Command::new("reg")
            .args(&["add", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "BIRVaultDaemon", "/t", "REG_SZ", "/d", &daemon_path_str, "/f"])
            .output();
            
        // Start it immediately
        let _ = std::process::Command::new(&daemon_path).spawn();
        info!("Windows Registry Run key added");
    }
}

#[cfg(target_os = "windows")]
fn uninstall_impl() {
    let _ = std::process::Command::new("reg")
        .args(&["delete", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "BIRVaultDaemon", "/f"])
        .output();
        
    // Kill the process if running
    let _ = std::process::Command::new("taskkill")
        .args(&["/F", "/IM", "bir-daemon.exe"])
        .output();
    info!("Windows Registry Run key removed");
}

#[cfg(target_os = "linux")]
fn install_impl() {
    if let Ok(exe_path) = env::current_exe() {
        let daemon_path = exe_path.parent().unwrap().join("bir-daemon");
        
        let service_content = format!(r#"[Unit]
Description=BIR Vault Background Daemon
After=network.target

[Service]
Type=simple
ExecStart={}
Restart=always
RestartSec=10

[Install]
WantedBy=default.target"#, daemon_path.display());

        let home_dir = std::env::var("HOME").expect("HOME not set");
        let systemd_dir = std::path::Path::new(&home_dir).join(".config").join("systemd").join("user");
        let _ = std::fs::create_dir_all(&systemd_dir);
        let service_path = systemd_dir.join("bir-vault-daemon.service");
        
        if let Err(e) = std::fs::write(&service_path, service_content) {
            warn!("Failed to write systemd service: {}", e);
            return;
        }

        let _ = std::process::Command::new("systemctl").args(&["--user", "daemon-reload"]).output();
        let _ = std::process::Command::new("systemctl").args(&["--user", "enable", "--now", "bir-vault-daemon.service"]).output();
        info!("Linux systemd service installed");
    }
}

#[cfg(target_os = "linux")]
fn uninstall_impl() {
    let _ = std::process::Command::new("systemctl").args(&["--user", "disable", "--now", "bir-vault-daemon.service"]).output();
    
    let home_dir = std::env::var("HOME").unwrap_or_default();
    let service_path = std::path::Path::new(&home_dir).join(".config").join("systemd").join("user").join("bir-vault-daemon.service");
    let _ = std::fs::remove_file(service_path);
    
    let _ = std::process::Command::new("systemctl").args(&["--user", "daemon-reload"]).output();
    info!("Linux systemd service uninstalled");
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn install_impl() {
    warn!("Daemon installation not supported on this platform.");
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn uninstall_impl() {
    warn!("Daemon uninstallation not supported on this platform.");
}
