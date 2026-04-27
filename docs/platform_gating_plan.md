# Implementation Plan: Platform Gating Standardization

This implementation plan details the steps required to standardize platform-specific code gating (OS and features) across the `bir-core`, `bir-desktop`, and `bir-print` crates. This ensures the application compiles and functions correctly across macOS, Windows, and Linux.

## Phase 1: Architectural Foundation (Platform Modules)

**Goal:** Establish the `platform` module structure to encapsulate OS-specific implementation details, preventing `#[cfg]` block scattering in business and UI logic.

1.  **Create `bir-core/src/platform/` Module**
    *   Create directory `crates/bir-core/src/platform/`.
    *   Create `mod.rs`, `macos.rs`, `windows.rs`, and `linux.rs`.
    *   In `mod.rs`, add the standard GPUI platform gating pattern:
        ```rust
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
        ```
    *   Update `crates/bir-core/src/lib.rs` to include `pub mod platform;`.

2.  **Create `bir-desktop/src/platform/` Module**
    *   Follow the exact same structure as Step 1, but for UI-specific platform integrations (e.g., global keybindings).
    *   Update `crates/bir-desktop/src/main.rs` (or `app.rs` if appropriate) to include the new module.

## Phase 2: Refactoring Core Business Logic (`bir-core`)

**Goal:** Move scattered OS-specific logic into the new `platform` module APIs.

1.  **Abstract Database Pathing (`crates/bir-core/src/db.rs`)**
    *   Extract the `cfg!(target_os = "macos")` logic from `default_database_path()`.
    *   Implement `pub fn app_data_dir() -> PathBuf` in `platform/macos.rs` (returning `~/Library/Application Support/...`), `platform/windows.rs`, and `platform/linux.rs` (returning generic paths like `~/.taxman-ebir`).
    *   Update `db.rs` to call `crate::platform::app_data_dir()`.

2.  **Abstract Keychain Access (`crates/bir-core/src/db.rs`)**
    *   Move the `get_or_create_master_key()` logic into the platform modules.
    *   In `platform/macos.rs`, implement `pub fn get_or_create_master_key() -> Result<String, DbError>` using the existing `Command::new("security")` logic.
    *   In `platform/windows.rs` and `platform/linux.rs`, implement the same function signature using the `keyring` crate.
    *   Update `db.rs` to call `crate::platform::get_or_create_master_key()`.

3.  **Segregate Daemon Installer (`crates/bir-core/src/daemon_installer.rs`)**
    *   This file currently contains monolithic gating. Move the logic into the respective platform modules.
    *   Define `pub fn install_daemon()` and `pub fn uninstall_daemon()` in all three OS files.
    *   **Feature Gating (`mas_build`):** In `platform/macos.rs`, use `#[cfg(feature = "mas_build")]` to provide a no-op implementation, and `#[cfg(not(feature = "mas_build"))]` for the `launchctl` implementation.
    *   Delete the old `daemon_installer.rs` and update references to use `crate::platform::install_daemon()`.

4.  **Fix Shell Execution in Cron Jobs (`crates/bir-core/src/background_cron.rs`)**
    *   **Issue:** `tokio::process::Command::new("sh").arg("-c").arg(cmd)` assumes a Unix-like environment, breaking on Windows.
    *   **Solution:** Abstract shell execution. Add `pub async fn run_shell_command(cmd: &str) -> std::process::Output` to the platform modules.
    *   `macos.rs` / `linux.rs`: Use `Command::new("sh").arg("-c")`.
    *   `windows.rs`: Use `Command::new("cmd").arg("/c")` or `Command::new("powershell").arg("-Command")`.

## Phase 3: Refactoring Desktop & UI (`bir-desktop` & `bir-print`)

**Goal:** Clean up UI code and fix cross-platform compilation blockers.

1.  **Centralize Keybindings (`crates/bir-desktop/src/main.rs`)**
    *   Move the inline `#[cfg]` keybinding registrations out of `main.rs`.
    *   Define `pub fn bind_global_keys(cx: &mut gpui::AppContext)` in `bir-desktop/src/platform/*.rs`.
    *   `macos.rs`: Register shortcuts using the `cmd` modifier.
    *   `windows.rs` / `linux.rs`: Register shortcuts using the `ctrl` modifier.
    *   Call `crate::platform::bind_global_keys(cx)` during application startup.

2.  **Fix macOS-Specific Print Commands (`crates/bir-desktop/src/views/pdf_viewer.rs`)**
    *   **Issue 1:** `PdfViewerView::reveal_pdf` hardcodes `Command::new("open").arg("-R")` (macOS only).
    *   **Issue 2:** `PdfViewerView::print_pdf` hardcodes `Command::new("swift")` (macOS only).
    *   **Solution:** Move these into the `bir-desktop/src/platform/` module as `pub fn reveal_file(path: &Path)` and `pub fn print_pdf(path: &Path)`.
    *   Provide Windows/Linux fallback implementations (e.g., using `open::that()` for reveal, and perhaps a no-op or specific command for print on Windows/Linux).

3.  **Resolve Hard Dependencies in Print Crate (`crates/bir-print/Cargo.toml`)**
    *   **Issue:** The `objc2` dependencies are unconditional, causing Windows/Linux builds to fail immediately during compilation.
    *   **Solution:** Update `bir-print/Cargo.toml` to gate macOS-specific dependencies:
        ```toml
        [target.'cfg(target_os = "macos")'.dependencies]
        objc2 = "0.5.2"
        objc2-foundation = { version = "0.2.2", features = ["NSString"] }
        objc2-app-kit = { version = "0.2.2", features = ["NSApplication", "NSPrintInfo", "NSPrintOperation"] }
        objc2-web-kit = { version = "0.2.2", features = ["WKWebView"] }
        ```
    *   In `bir-print/src/lib.rs`, wrap the `print_html_mac` function (even if commented out) and any related `objc2` imports in `#[cfg(target_os = "macos")]`.

## Phase 4: Final Validation

1.  Verify the project compiles successfully using:
    *   `cargo check` (Current OS)
    *   `cargo check --target x86_64-pc-windows-msvc` (if cross-compilation toolchains are available, or trust the gating).
2.  Ensure no `cfg!(target_os = "...")` or inline `#[cfg(...)]` blocks remain in core business logic or UI rendering functions.
