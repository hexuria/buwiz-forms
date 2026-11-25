//! Platform-specific implementations for OS services.
//!
//! This module hides all `#[cfg(target_os)]` gating behind a clean public API.
//! Consumers call `crate::platform::data_dir()`, `crate::platform::run_shell_command()`,
//! etc. without knowing which OS they are on.

/// App id of the NSH app. In the plain-SQLite build (cargo feature `sqlcipher`
/// off) `data_dir()` is a folder named after it, never the old app's group
/// container, so the two apps run side by side.
pub const NSH_APP_ID: &str = "dev.goldcoders.bir";

/// Environment switch that turns every OS keychain read and write off.
pub const NO_KEYCHAIN_ENV: &str = "BUWIZ_NO_KEYCHAIN";

/// `BUWIZ_NO_KEYCHAIN=1` is set: every keychain/keyring read in bir-core
/// answers "not found" and every write fails, without calling the OS. Dev,
/// test and agent runs always set it. Any value except empty, `0`, `false`,
/// `no` or `off` switches the keychain off, so a typo cannot reach the OS.
pub fn keychain_disabled() -> bool {
    cfg!(target_os = "android")
        || keychain_switch_is_on(crate::config::var(NO_KEYCHAIN_ENV).ok().as_deref())
}

pub(crate) fn keychain_switch_is_on(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        )
    })
}

/// Opens a web address in the system browser. The embedding app registers
/// one on phones ([`set_url_opener`]), where bir-core cannot reach the
/// browser itself: UIKit on iOS, an `ACTION_VIEW` intent on Android.
pub type UrlOpener = fn(&str) -> Result<(), String>;

static URL_OPENER: std::sync::OnceLock<UrlOpener> = std::sync::OnceLock::new();

/// Registers the app's browser opener, once. `false` when one was already
/// registered (the first one stays).
pub fn set_url_opener(opener: UrlOpener) -> bool {
    URL_OPENER.set(opener).is_ok()
}

/// The registered browser opener, if the app gave one.
pub fn url_opener() -> Option<UrlOpener> {
    URL_OPENER.get().copied()
}

/// Reads the platform's own device id where bir-core cannot: `Settings.Secure.ANDROID_ID` on Android, through the
/// app's Java access. Only ever hashed ([`crate::install_id`]); `None` when it cannot be read.
pub type DeviceIdSource = fn() -> Option<String>;

static DEVICE_ID_SOURCE: std::sync::OnceLock<DeviceIdSource> = std::sync::OnceLock::new();

/// Registers the app's device id reader, once. `false` when one was already registered (the first one stays).
pub fn set_device_id_source(source: DeviceIdSource) -> bool {
    DEVICE_ID_SOURCE.set(source).is_ok()
}

/// The registered device id reader, if the app gave one.
pub fn device_id_source() -> Option<DeviceIdSource> {
    DEVICE_ID_SOURCE.get().copied()
}

/// Whether a registered opener may be handed `url`: an `https://` address
/// with a host, or an `http://` one to this device itself (`localhost`,
/// `127.0.0.0/8`, `::1`) for a local development server. Nothing else (no
/// other scheme, no plain `http://` to the network, no credentials in the
/// address) leaves the app this way.
pub fn browser_url_allowed(url: &str) -> bool {
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return false;
    }
    let Some(host) = parsed.host_str().filter(|host| !host.is_empty()) else {
        return false;
    };
    match parsed.scheme() {
        "https" => true,
        "http" => {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        }
        _ => false,
    }
}

/// Run once at launch, before the database is opened. iOS: the data
/// folder is excluded from backups and given the file protection "complete
/// until first user authentication" (files made in it take it too), and the
/// three Keychain items one earlier build stored are deleted once
/// (`bir_submit::ios_storage::remove_old_items`; no Keychain call with
/// `BUWIZ_NO_KEYCHAIN=1`). Every other system: nothing.
///
/// It runs at module registration, before logging is installed, so its
/// outcome is recorded here and logged by [`log_app_storage`] once logging
/// exists.
pub fn prepare_app_storage() {
    #[cfg(target_os = "ios")]
    {
        let (warn, line) = ios::prepare_ios_storage();
        LAUNCH_STORAGE.record(warn, line);
    }
}

/// The outcome of [`prepare_app_storage`], until it is logged.
static LAUNCH_STORAGE: Pending = Pending::new();

/// Log the outcome of [`prepare_app_storage`], once: call it right after
/// logging is installed. A call before anything was recorded logs nothing
/// and loses nothing; after the line is out, every call is a no-op.
pub fn log_app_storage() {
    LAUNCH_STORAGE.flush(|warn, line| {
        if warn {
            tracing::warn!("{line}");
        } else {
            tracing::info!("{line}");
        }
    });
}

/// A log line made before logging exists, logged once when it does.
struct Pending {
    line: std::sync::Mutex<Option<(bool, String)>>,
}

impl Pending {
    const fn new() -> Self {
        Self {
            line: std::sync::Mutex::new(None),
        }
    }

    /// Keep `line` (`warn`: at warning level) until [`Pending::flush`].
    #[cfg_attr(not(any(target_os = "ios", test)), allow(dead_code))]
    fn record(&self, warn: bool, line: String) {
        *self
            .line
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((warn, line));
    }

    /// Hand the kept line to `emit` and forget it: at most once per record.
    fn flush(&self, emit: impl FnOnce(bool, &str)) {
        let taken = self
            .line
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some((warn, line)) = taken {
            emit(warn, &line);
        }
    }
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

#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "ios")]
pub use ios::*;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::*;

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

    /// Off iOS the launch step does nothing: no folder made, no marker
    /// written, under a home where the data folder would be.
    #[cfg(not(target_os = "ios"))]
    #[test]
    fn the_launch_storage_step_does_nothing_off_ios() {
        let home = tempfile::tempdir().unwrap();
        temp_env::with_var("HOME", Some(home.path()), prepare_app_storage);
        assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
    }

    /// The launch outcome is kept until logging exists and logged exactly
    /// once: a flush before it was recorded loses nothing, and later flushes
    /// log nothing more.
    #[test]
    fn the_launch_outcome_is_logged_once_however_late_logging_starts() {
        let pending = Pending::new();
        let mut logged = Vec::new();
        pending.flush(|warn, line| logged.push((warn, line.to_owned())));
        assert!(logged.is_empty(), "nothing recorded yet, nothing logged");
        pending.record(false, "App storage at launch: …".into());
        for _ in 0..3 {
            pending.flush(|warn, line| logged.push((warn, line.to_owned())));
        }
        assert_eq!(logged, [(false, "App storage at launch: …".to_owned())]);
        pending.record(true, "App storage at launch: warning".into());
        pending.flush(|warn, line| logged.push((warn, line.to_owned())));
        assert_eq!(logged.len(), 2);
        assert!(logged[1].0, "a warning stays a warning");
    }

    #[test]
    fn a_phone_opens_only_https_or_loopback_http_in_the_browser() {
        for allowed in [
            "https://account.buwiz.com/login/device?user_code=ABCD-EFGH",
            "HTTPS://Account.Buwiz.com/",
            "https://203.0.113.7/login",
            "http://localhost:3008/login/device",
            "http://LOCALHOST/",
            "http://127.0.0.1:3008/",
            "http://127.1/",
            "http://[::1]:3008/",
        ] {
            assert!(browser_url_allowed(allowed), "{allowed}");
        }
        for refused in [
            "http://account.buwiz.com/login",
            "http://10.0.2.2:3008/login",
            "http://localhost.evil.example/",
            "https://user:secret@account.buwiz.com/",
            "https://user@account.buwiz.com/",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "intent://scan/#Intent;scheme=zxing;end",
            "mailto:support@buwiz.com",
            "tel:123",
            "https://account.buwiz.com/a b",
            "https://account.buwiz.com/\n",
            "not a url",
            "",
        ] {
            assert!(!browser_url_allowed(refused), "{refused}");
        }
    }

    #[test]
    fn keychain_disabled_follows_buwiz_no_keychain() {
        temp_env::with_var(NO_KEYCHAIN_ENV, Some("1"), || {
            assert!(keychain_disabled());
        });
    }

    #[cfg(target_os = "android")]
    #[test]
    fn android_keychain_is_disabled_even_when_the_switch_is_off() {
        temp_env::with_var(NO_KEYCHAIN_ENV, Some("0"), || {
            assert!(keychain_disabled());
        });
    }
}
