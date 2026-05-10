//! Cross-platform service installer for the background daemon.
//!
//! This is a thin compatibility shim that delegates to the platform-specific
//! implementations in [`crate::platform`]. New code should call
//! `crate::platform::install_daemon()` / `crate::platform::uninstall_daemon()`
//! directly.

/// Install the background service to run automatically on user login.
pub fn install() {
    crate::platform::install_daemon();
}

/// Uninstall the background service.
pub fn uninstall() {
    crate::platform::uninstall_daemon();
}

/// Check if the background service is running.
pub fn is_daemon_running() -> bool {
    crate::platform::is_daemon_running()
}
