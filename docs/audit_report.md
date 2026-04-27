# eBIRForms Platform Gating Audit Report

This report outlines the current state of platform-specific code gating (OS and features) across the app, compared against the new architectural standard. 

## 1. Current State of Platform Gating

After scanning the codebase across the `crates/bir-core` and `crates/bir-desktop` directories, here are the areas that currently use `#[cfg]` or runtime `cfg!()` macros:

### A. Keybindings (`crates/bir-desktop/src/main.rs`)
- **Status:** Uses inline `#[cfg(target_os = "macos")]` and `#[cfg(not(target_os = "macos"))]` to bind `cmd-*` for macOS and `ctrl-*` for Linux/Windows.
- **Issue:** The UI code in `main.rs` is cluttered with `cfg` gates, violating the rule: *"never scatter `#[cfg]` everywhere in UI code"*.

### B. Database Paths (`crates/bir-core/src/db.rs`)
- **Status:** Uses runtime `cfg!(target_os = "macos")` in `default_database_path()` to decide between `~/Library/Application Support/Taxman/eBIRForms` and `~/.taxman-ebir`.
- **Issue:** Runtime checks are fine per the guidelines, but isolating this to a platform module makes it cleaner.

### C. OS Keychain Access (`crates/bir-core/src/db.rs`)
- **Status:** Uses `#[cfg(target_os = "macos")]` to use the `security` CLI, and `#[cfg(not(target_os = "macos"))]` to use the `keyring` crate.
- **Issue:** Clutters core business logic (`db.rs`) with platform-specific command executions and dependencies.

### D. Daemon Installer (`crates/bir-core/src/daemon_installer.rs`)
- **Status:** Uses heavy inline gating for macOS (`launchctl`), Windows (`reg`), Linux (`systemctl`), and `mas_build` feature gates.
- **Issue:** While nicely contained in one file, it doesn't follow the cleanly segregated `platform/macos.rs`, `platform/windows.rs` module pattern.

### E. Dependencies (`Cargo.toml`)
- **Status:** `keyring = { version = "3", features = ["apple-native"] }` is set universally in the workspace `Cargo.toml`. 
- **Issue:** This forces the `apple-native` feature to resolve on all platforms. We should use `[target.'cfg(target_os = "macos")'.dependencies]` if we want strict platform isolation, or isolate it inside the platform modules.

---

## 2. Refactoring Plan & Recommendations

To adopt the two-layer module and feature-gate pattern, we should implement the following architectural changes:

### A. Create the `platform` module
Create the structure:
```text
crates/bir-core/src/platform/
  mod.rs
  macos.rs
  linux.rs
  windows.rs
```
And similarly for `crates/bir-desktop` if UI-specific platform code is needed.

### B. Move Keybindings to Platform Module
Instead of gating in `main.rs`, expose a unified API:
```rust
// In crates/bir-desktop/src/platform/macos.rs
pub fn bind_global_keys(cx: &mut AppContext) {
    cx.bind_keys([ KeyBinding::new("cmd-enter", SubmitCurrentForm, None), ... ]);
}

// In crates/bir-desktop/src/platform/windows.rs
pub fn bind_global_keys(cx: &mut AppContext) {
    cx.bind_keys([ KeyBinding::new("ctrl-enter", SubmitCurrentForm, None), ... ]);
}
```
Then in `main.rs`:
```rust
crate::platform::bind_global_keys(cx);
```

### C. Abstract Keychain & Data Directories
Move `get_or_create_master_key()` and `default_database_path()` into `crates/bir-core/src/platform/...`.
```rust
// crates/bir-core/src/platform/macos.rs
pub fn data_dir() -> PathBuf { ... }
pub fn get_master_key() -> Result<String, DbError> { ... }
```
Then `db.rs` simply calls `crate::platform::get_master_key()`.

### D. Segregate the Daemon Installer
Split `daemon_installer.rs` into the respective `platform/*.rs` files. Each file will implement `pub fn install_daemon()` and `pub fn uninstall_daemon()`.

### E. Feature Gating `mas_build`
Currently, `mas_build` is defined in `crates/bir-core/Cargo.toml`. It should be cleanly handled via combining OS and feature gates inside the `platform/macos.rs` module:
```rust
#[cfg(feature = "mas_build")]
pub fn install_daemon() {
    info!("App Store build: Skipping LaunchAgent installation.");
}

#[cfg(not(feature = "mas_build"))]
pub fn install_daemon() {
    // LaunchAgent logic
}
```

## Summary
The app is currently functional but violates the clean architecture rules by scattering `#[cfg]` blocks directly within core domains (`db.rs`) and application roots (`main.rs`). Adopting the `platform/` module pattern will immediately isolate OS dependencies, hide the App Store (`mas_build`) behavior behind clean APIs, and keep the GPUI UI logic completely platform-agnostic.
