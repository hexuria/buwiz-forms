//! Platform-specific UI integrations (keybindings, file operations, printing).
//!
//! This module hides all `#[cfg(target_os)]` gating behind a clean public API.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

use std::path::PathBuf;

/// Dynamically locates application resources across local dev, macOS bundles, and Linux tarballs.
pub fn find_resource_dir(name: &str) -> PathBuf {
    let current_exe = std::env::current_exe().unwrap_or_default();
    if let Some(parent) = current_exe.parent() {
        // 1. Next to executable (Linux release / Windows install / local run if copied)
        let local = parent.join(name);
        if local.exists() {
            return local;
        }

        // 2. macOS .app bundle (Resources is sibling to MacOS)
        let macos_bundle = parent.join("../Resources").join(name);
        if macos_bundle.exists() {
            return macos_bundle.canonicalize().unwrap_or(macos_bundle);
        }

        // 3. Cargo workspace root (fallback for `cargo run` where exe is in target/debug/ or target/release/)
        let cargo_workspace = parent.join("../../..").join(name);
        if cargo_workspace.exists() {
            return cargo_workspace.canonicalize().unwrap_or(cargo_workspace);
        }
    }

    // Fallback to CWD
    std::env::current_dir().unwrap_or_default().join(name)
}

// Re-export common functions
pub use hide_from_dock;
pub use show_in_dock;
