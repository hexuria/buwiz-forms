//! Build script for bir-core.
//!
//! Phase 1: Platform-specific library linking (keeping only this phase).

fn main() {
    // ── Phase 1: Platform linking ──
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        println!("cargo:rustc-link-lib=dylib=user32");
        println!("cargo:rustc-link-lib=dylib=gdi32");
        println!("cargo:rustc-link-lib=dylib=crypt32");
        println!("cargo:rustc-link-lib=dylib=advapi32");
        println!("cargo:rustc-link-lib=dylib=ws2_32");
        println!("cargo:rustc-link-lib=dylib=userenv");
        println!("cargo:rustc-link-lib=dylib=bcrypt");
    }

    // Compile-time server and account origins. Not secrets. `option_env!` in
    // `cloud_account` reads them; emitting them here is what makes a later
    // build pick up a new value instead of a cached one.
    for name in ["BUWIZ_SERVER_URL", "BUWIZ_ACCOUNT_URL"] {
        println!("cargo:rerun-if-env-changed={name}");
        if let Ok(value) = std::env::var(name) {
            let value = value.replace(['\n', '\r'], "");
            if !value.trim().is_empty() {
                println!("cargo:rustc-env={name}={value}");
            }
        }
    }
}
