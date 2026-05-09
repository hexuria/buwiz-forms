//! macOS-specific implementations for bir-core platform services.
#![allow(unexpected_cfgs)]

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

#[cfg(target_os = "macos")]
#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}

#[cfg(target_os = "macos")]
#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {}

pub fn install_daemon() {
    #[cfg(target_os = "macos")]
    {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};

        let cls = objc::runtime::Class::get("SMAppService");
        let Some(cls) = cls else {
            warn!("SMAppService is not available on this macOS version.");
            return;
        };

        unsafe {
            let plist_name: *mut Object = msg_send![class!(NSString), stringWithUTF8String: c"com.bir.vault.daemon.plist".as_ptr()];
            let service: *mut Object = msg_send![cls, agentServiceWithPlistName: plist_name];

            let mut error: *mut Object = std::ptr::null_mut();
            let success: bool = msg_send![service, registerAndReturnError: &mut error];

            if success {
                info!("macOS SMAppService LaunchAgent registered successfully");
            } else {
                let err_desc: *mut Object = msg_send![error, localizedDescription];
                if !err_desc.is_null() {
                    let err_str: *const std::ffi::c_char = msg_send![err_desc, UTF8String];
                    if !err_str.is_null() {
                        let err_msg = std::ffi::CStr::from_ptr(err_str).to_string_lossy();
                        warn!("Failed to register macOS SMAppService: {}", err_msg);
                        return;
                    }
                }
                warn!("Failed to register macOS SMAppService (unknown error)");
            }
        }
    }
}

pub fn uninstall_daemon() {
    #[cfg(target_os = "macos")]
    {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};

        let cls = objc::runtime::Class::get("SMAppService");
        let Some(cls) = cls else {
            return;
        };

        unsafe {
            let plist_name: *mut Object = msg_send![class!(NSString), stringWithUTF8String: c"com.bir.vault.daemon.plist".as_ptr()];
            let service: *mut Object = msg_send![cls, agentServiceWithPlistName: plist_name];

            let mut error: *mut Object = std::ptr::null_mut();
            let success: bool = msg_send![service, unregisterAndReturnError: &mut error];

            if success {
                info!("macOS SMAppService LaunchAgent unregistered successfully");
            } else {
                warn!("Failed to unregister macOS SMAppService LaunchAgent");
            }
        }
    }
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
