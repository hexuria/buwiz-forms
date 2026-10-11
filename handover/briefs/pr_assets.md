Running a debug build whose `CARGO_TARGET_DIR` is outside the repo (e.g. a shared cache dir) from another working directory shows **no sidebar logo, no icons, and an empty profile-actions button**. The assets are fine. `find_resource_dir` never finds `assets/`, and the asset loader fails every load silently.

## Change (`crates/bir-desktop/src/platform/mod.rs`)
- **Debug fallback:** debug builds also try `CARGO_MANIFEST_DIR/../../assets`, the workspace's own assets. This is `#[cfg(debug_assertions)]` only, so release binaries never point at a build-machine path. Packaged Linux builds keep using `/usr/share/ebirforms/assets` (from `[package.metadata.deb] assets`).
- **Warning:** when no candidate exists, `find_resource_dir` logs a `tracing::warn!` with the candidates it tried, so a missing-assets run is diagnosable instead of silently blank.
- **Test:** a debug build includes the manifest-dir candidate, and that candidate exists.

## Verification
- `cargo clippy -p bir-desktop --all-targets --features dev-tools,agent -- -D warnings`: clean
- `cargo clippy -p bir-desktop -- -D warnings`: clean
- `cargo test -p bir-desktop --lib platform`: 3 passed

Found while running the app under Xvfb/noVNC on Linux (aarch64).

🤖 Generated with [Claude Code](https://claude.com/claude-code)
