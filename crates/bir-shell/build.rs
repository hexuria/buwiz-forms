//! Windows/MSVC linker policy for the shipped `bir-shell` GUI binary.
//!
//! Issue #96 was the painted app's overflow; the shell walks the same
//! recursive GPUI element tree in `request_layout`, and it additionally runs
//! QuickJS — JavaScript recursion consumes native stack, so the shipped
//! binary carries the same 8 MiB main-thread reserve as `bir-shell`.
//!
//! Only the `bir-shell` bin gets the flag: `cargo:rustc-link-arg-bin` is
//! per-binary, so tests and other workspace binaries keep the platform
//! default. Non-MSVC targets do not understand `/STACK`, so this is emitted
//! only for Windows + MSVC.

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ENV");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        const STACK_RESERVE: u64 = 8 * 1024 * 1024;
        println!("cargo:rustc-link-arg-bin=bir-shell=/STACK:{STACK_RESERVE}");
    }
}
