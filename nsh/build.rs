//! The app's main window title for `os_login::APP_WINDOW_TITLE` (PLAN W4): nsh.json `window.title`, the value the kit
//! gives the main window, read once at build time so there is no second copy of the string. A missing or empty title
//! stops the build: an empty one would match no window and silently make Windows Hello unavailable for everyone.

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=nsh.json");
    let manifest_dir =
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let path = Path::new(&manifest_dir).join("nsh.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    let config: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("parsing {}: {error}", path.display()));
    let title = config
        .pointer("/window/title")
        .and_then(serde_json::Value::as_str)
        .filter(|title| !title.trim().is_empty() && !title.contains(['\n', '\r']))
        .unwrap_or_else(|| {
            panic!(
                "{} needs window.title: a non-empty, one-line string",
                path.display()
            )
        });
    println!("cargo:rustc-env=APP_WINDOW_TITLE={title}");
}
