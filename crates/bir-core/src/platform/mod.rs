//! Platform-specific implementations for OS services.
//!
//! This module hides all `#[cfg(target_os)]` gating behind a clean public API.
//! Consumers call `crate::platform::data_dir()`, `crate::platform::run_shell_command()`,
//! etc. without knowing which OS they are on.

/// App id of the NSH app. In the plain-SQLite build (cargo feature `sqlcipher`
/// off) `data_dir()` is a folder named after it, never the old app's group
/// container, so the two apps run side by side.
pub const NSH_APP_ID: &str = "dev.goldcoders.bir.nsh";

/// Environment switch that turns every OS keychain read and write off.
pub const NO_KEYCHAIN_ENV: &str = "BUWIZ_NO_KEYCHAIN";

/// `BUWIZ_NO_KEYCHAIN=1` is set: every keychain/keyring read in bir-core
/// answers "not found" and every write fails, without calling the OS. Dev,
/// test and agent runs always set it. Any value except empty, `0`, `false`,
/// `no` or `off` switches the keychain off, so a typo cannot reach the OS.
pub fn keychain_disabled() -> bool {
    keychain_switch_is_on(std::env::var(NO_KEYCHAIN_ENV).ok().as_deref())
}

pub(crate) fn keychain_switch_is_on(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        )
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keychain_switch_reads_the_environment_value() {
        assert!(keychain_switch_is_on(Some("1")));
        assert!(keychain_switch_is_on(Some(" 1 ")));
        assert!(keychain_switch_is_on(Some("true")));
        assert!(keychain_switch_is_on(Some("yes")));
        assert!(!keychain_switch_is_on(None));
        assert!(!keychain_switch_is_on(Some("")));
        assert!(!keychain_switch_is_on(Some("0")));
        assert!(!keychain_switch_is_on(Some("false")));
        assert!(!keychain_switch_is_on(Some("Off")));
    }

    #[test]
    fn keychain_disabled_follows_buwiz_no_keychain() {
        temp_env::with_var(NO_KEYCHAIN_ENV, Some("1"), || {
            assert!(keychain_disabled());
        });
    }
}
