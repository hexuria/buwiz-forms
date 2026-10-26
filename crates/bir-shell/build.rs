//! Windows/MSVC linker policy for the shipped `bir-shell` GUI binary.
//!
//! Issue #96 was the painted app's overflow; the shell walks the same
//! recursive GPUI element tree in `request_layout`, and it additionally runs
//! QuickJS — JavaScript recursion consumes native stack, so the shipped
//! binary carries the same 8 MiB main-thread reserve as `bir-desktop`.
//!
//! Only the `bir-shell` bin gets the flag: `cargo:rustc-link-arg-bin` is
//! per-binary, so tests and other workspace binaries keep the platform
//! default. Non-MSVC targets do not understand `/STACK`, so this is emitted
//! only for Windows + MSVC.
//!
//! It also embeds `mini-apps/` (the TSX apps `bir-mini-app` serves as MCP
//! resources) so the release binary is self-contained: one
//! `(stem, include_str!)` entry per `*.tsx`, in name order.

use std::path::Path;

fn main() {
    embed_mini_apps();
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ENV");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        const STACK_RESERVE: u64 = 8 * 1024 * 1024;
        println!("cargo:rustc-link-arg-bin=bir-shell=/STACK:{STACK_RESERVE}");
    }
}

fn embed_mini_apps() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let dir = Path::new(&manifest).join("mini-apps");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut apps: Vec<(String, String)> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "tsx"))
                .filter_map(|path| {
                    let stem = path.file_stem()?.to_str()?.to_string();
                    Some((stem, path.display().to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    apps.sort();
    let mut out = String::from("&[\n");
    for (stem, path) in &apps {
        println!("cargo:rerun-if-changed={path}");
        out.push_str(&format!("    ({stem:?}, include_str!({path:?})),\n"));
    }
    out.push_str("]\n");
    let target =
        Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("mini_apps.rs");
    std::fs::write(target, out).expect("mini_apps.rs is written");
}
