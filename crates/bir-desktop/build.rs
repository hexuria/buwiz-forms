//! Windows/MSVC linker policy for the shipped `bir-desktop` GUI binary.
//!
//! Issue #96: the default 1 MiB main-thread stack reserve overflows in debug
//! builds during normal navigation (taxpayer sidebar -> dashboard render).
//! The GPUI element tree recurses per nesting level inside `request_layout`,
//! and debug frames are unoptimized, so the desktop binary needs a larger
//! reserve. Release builds survive at 1 MiB today, but they walk the same
//! recursive layout path; the reserve is policy for the shipped binary, not
//! a per-profile workaround. Measured data and the chosen margin are recorded
//! in `docs/plans/windows-hardening.md` (W01).
//!
//! Only the `bir-desktop` bin gets the flag: `cargo:rustc-link-arg-bin` is
//! per-binary, so `bir-headless`, tests and other workspace binaries keep the
//! platform default. Non-MSVC targets do not understand `/STACK`, so this is
//! emitted only for Windows + MSVC.

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ENV");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        const STACK_RESERVE: u64 = 8 * 1024 * 1024;
        println!("cargo:rustc-link-arg-bin=bir-desktop=/STACK:{STACK_RESERVE}");
    }
}
