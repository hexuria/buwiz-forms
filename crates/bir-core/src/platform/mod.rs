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
///
/// A test build (bir-core's own tests, or another crate's through bir-core's
/// `test-support` feature) has the keychain off whatever the switch says: a
/// test never reads, writes or deletes the person's real keychain items.
pub fn keychain_disabled() -> bool {
    cfg!(any(test, feature = "test-support", target_os = "android"))
        || keychain_switch_is_on(crate::config::var(NO_KEYCHAIN_ENV).ok().as_deref())
}

/// The app's data folder.
///
/// In a test build (bir-core's own tests, or another crate's through
/// bir-core's `test-support` feature, never an app or release build) a data
/// folder outside the system temp folder is never used: a test that did not
/// point `HOME` (`XDG_DATA_HOME`, `LOCALAPPDATA`) at a temp folder of its own
/// gets this process's scratch folder instead, so nothing a test writes,
/// renames or deletes lands in the person's real data folder. Every build
/// that ships answers the system's folder, as before.
pub fn data_dir() -> std::path::PathBuf {
    // The system's folder is only looked at here, never created: a test build that does not use it leaves it alone.
    #[cfg(all(any(test, feature = "test-support"), not(target_os = "android")))]
    if !in_temp_folder(&os_data_dir_path()) {
        return test_data_dir();
    }
    os_data_dir()
}

/// The system's data folder for this app, without the test-build rule of [`data_dir`].
pub(crate) fn os_data_dir() -> std::path::PathBuf {
    os::data_dir()
}

/// [`os_data_dir`] without creating it.
#[cfg(all(any(test, feature = "test-support"), not(target_os = "android")))]
pub(crate) fn os_data_dir_path() -> std::path::PathBuf {
    os::data_dir_path().unwrap_or_else(|| std::path::PathBuf::from(".taxman-ebir"))
}

/// `dir` created if missing; with no system folder (`None`) the old relative fallback `.taxman-ebir`, not created.
#[cfg(not(target_os = "android"))]
fn create_data_dir(dir: Option<std::path::PathBuf>) -> std::path::PathBuf {
    let Some(dir) = dir else {
        return std::path::PathBuf::from(".taxman-ebir");
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Whether `path` is inside the system temp folder. Both are resolved through their nearest existing folder, so a
/// path not made yet (`/var/…` on macOS, really `/private/var/…`) is judged as it will be once made.
#[cfg(all(any(test, feature = "test-support"), not(target_os = "android")))]
fn in_temp_folder(path: &std::path::Path) -> bool {
    fn resolve(path: &std::path::Path) -> std::path::PathBuf {
        let mut rest = Vec::new();
        let mut existing = path;
        loop {
            if let Ok(real) = existing.canonicalize() {
                return rest.iter().rev().fold(real, |dir, name| dir.join(name));
            }
            match (existing.parent(), existing.file_name()) {
                (Some(parent), Some(name)) => {
                    rest.push(name);
                    existing = parent;
                }
                _ => return path.to_path_buf(),
            }
        }
    }
    resolve(path).starts_with(resolve(&std::env::temp_dir()))
}

/// The scratch data folder of a test process: `<temp>/bir-test-data-<pid>`, emptied on its first use in the process.
#[cfg(all(any(test, feature = "test-support"), not(target_os = "android")))]
fn test_data_dir() -> std::path::PathBuf {
    static EMPTIED: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let dir = fresh_scratch_dir(&EMPTIED, "bir-test-data");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// `<temp>/<prefix>-<pid>` ([`temp_dir`]), emptied the first time `once` is asked in this process: an earlier
/// process with the same id may have left files there. Only that folder of the temp folder is ever removed, and only
/// when its name is exactly `<prefix>-<pid>`; a link there (a symlink, or on Windows any reparse point such as a
/// junction) is removed as a link, never followed.
pub(crate) fn fresh_scratch_dir(
    once: &std::sync::OnceLock<std::path::PathBuf>,
    prefix: &str,
) -> std::path::PathBuf {
    once.get_or_init(|| {
        let temp = temp_dir();
        let name = format!("{prefix}-{}", std::process::id());
        let dir = temp.join(&name);
        let own_folder = dir.parent() == Some(temp.as_path())
            && dir.file_name() == Some(std::ffi::OsStr::new(&name));
        if own_folder && let Ok(meta) = std::fs::symlink_metadata(&dir) {
            let _ = if meta.file_type().is_symlink() || is_reparse_point(&meta) {
                // A directory link (a Windows junction or directory symlink) goes with remove_dir, any other with
                // remove_file; neither follows it.
                std::fs::remove_dir(&dir).or_else(|_| std::fs::remove_file(&dir))
            } else if meta.is_dir() {
                std::fs::remove_dir_all(&dir)
            } else {
                std::fs::remove_file(&dir)
            };
        }
        dir
    })
    .clone()
}

/// Whether `meta` (from `symlink_metadata`) is a Windows reparse point: a junction, a symlink or another link.
#[cfg(windows)]
fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_meta: &std::fs::Metadata) -> bool {
    false
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

/// The device folder (accounts-and-lock PLAN §A.2): where the device-wide
/// files live, whichever room is open. The saved sign-in, the relay device
/// key, the install id, the pending email sign-in, `device.db` and the
/// one-copy guard's lock are found here, never from the database path.
///
/// - Single-room mode (a debug build or a test environment that named its
///   database, [`crate::db::single_room_database`]): that file's folder, so
///   every test keeps its scratch isolation.
/// - Else [`data_dir`]. On a normal install that is where those files always
///   were, so nothing moves.
pub fn device_dir() -> std::path::PathBuf {
    if let Some(named) = device_dir_override() {
        return named;
    }
    match crate::db::single_room_database() {
        Some(named) => named
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from(".")),
        None => data_dir(),
    }
}

thread_local! {
    /// A test's device folder (A3a's host tests): rooms, `device.db` and the legacy file of a
    /// scratch install, never the real one. Same thread discipline as the database-path override.
    static DEVICE_DIR_OVERRIDE: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// A test's device folder, or None. Test environments only: outside them the override is ignored.
pub fn set_device_dir_override(path: Option<std::path::PathBuf>) {
    DEVICE_DIR_OVERRIDE.with(|slot| *slot.borrow_mut() = path);
}

/// The device-folder override of this thread, captured for a spawned thread to resume — the
/// same discipline as the database-path override's: a worker thread that reads the account file
/// or the device key must read the same install as the thread that asked.
pub struct DeviceDirOverride {
    path: Option<std::path::PathBuf>,
}

impl DeviceDirOverride {
    /// Run `work` with the captured override set on this thread, restoring the thread's own
    /// after. In production there is no override and this is a pass-through.
    pub fn run<T>(&self, work: impl FnOnce() -> T) -> T {
        let previous = DEVICE_DIR_OVERRIDE.with(|slot| slot.borrow().clone());
        DEVICE_DIR_OVERRIDE.with(|slot| *slot.borrow_mut() = self.path.clone());
        let answer = work();
        DEVICE_DIR_OVERRIDE.with(|slot| *slot.borrow_mut() = previous);
        answer
    }
}

/// Capture this thread's device-folder override for a spawned thread.
pub fn capture_device_dir_override() -> DeviceDirOverride {
    DeviceDirOverride {
        path: DEVICE_DIR_OVERRIDE.with(|slot| slot.borrow().clone()),
    }
}

/// Whether a test device-folder override is set (A3a's host tests): then the legacy single-file
/// slot never opens — outside single-room mode the room is the only slot.
pub fn device_dir_override_active() -> bool {
    device_dir_override().is_some()
}

fn device_dir_override() -> Option<std::path::PathBuf> {
    if !crate::db::test_environment() {
        return None;
    }
    DEVICE_DIR_OVERRIDE.with(|slot| slot.borrow().clone())
}

/// The folder [`prepare_app_storage`] excludes from backups on iOS and gives
/// file protection: the whole [`data_dir`]. Every new folder of the rooms
/// layout (`rooms/`, `backups/`, `previews/`, `device.db`) is made inside it
/// (Lead's review point 1), never beside it.
pub fn backup_excluded_dir() -> std::path::PathBuf {
    data_dir()
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

// Each system's `data_dir` is shadowed by [`data_dir`] above and reached as `os::data_dir`.
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as os;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as os;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as os;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "ios")]
use ios as os;
#[cfg(target_os = "ios")]
pub use ios::*;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
use android as os;
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

    #[test]
    fn a_test_build_has_the_keychain_off_even_when_the_switch_is_off() {
        temp_env::with_var(NO_KEYCHAIN_ENV, Some("0"), || {
            assert!(keychain_disabled());
        });
    }

    /// A test build never uses a data folder outside the system temp folder: with the account's own home it gets a
    /// scratch folder, never the real one; a test that points the home at a temp folder keeps that folder.
    #[cfg(not(target_os = "android"))]
    #[test]
    fn a_test_build_never_uses_the_real_data_folder() {
        let temp = std::env::temp_dir().canonicalize().unwrap();
        let real = os_data_dir_path();
        let dir = data_dir();
        assert!(
            dir.canonicalize().unwrap().starts_with(&temp),
            "{}",
            dir.display()
        );
        assert!(
            in_temp_folder(&real) || (dir != real && !dir.starts_with(&real)),
            "{} is the real data folder {}",
            dir.display(),
            real.display()
        );

        let home = tempfile::tempdir().unwrap();
        temp_env::with_vars(
            [
                ("HOME", Some(home.path().as_os_str())),
                ("XDG_DATA_HOME", None),
                ("LOCALAPPDATA", Some(home.path().as_os_str())),
            ],
            || assert_eq!(data_dir(), os_data_dir()),
        );
    }

    /// The system's data folder is named without being made; only [`os_data_dir`] makes it.
    #[cfg(not(target_os = "android"))]
    #[test]
    fn the_system_data_folder_is_named_without_being_made() {
        let home = tempfile::tempdir().unwrap();
        temp_env::with_vars(
            [
                ("HOME", Some(home.path().as_os_str())),
                ("XDG_DATA_HOME", None),
                ("LOCALAPPDATA", Some(home.path().as_os_str())),
            ],
            || {
                let path = os_data_dir_path();
                assert!(path.starts_with(home.path()), "{}", path.display());
                assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
                assert_eq!(os_data_dir(), path);
                assert!(path.is_dir());
            },
        );
    }

    /// A scratch folder an earlier process with this id left is emptied on its first use, and only then; a prefix
    /// that would name a folder deeper in the temp folder removes nothing.
    #[test]
    fn a_scratch_folder_is_emptied_once_per_process() {
        let prefix = "bir-test-scratch-check";
        let dir = temp_dir().join(format!("{prefix}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("old")).unwrap();
        std::fs::write(dir.join("old").join("install_id"), "stale").unwrap();
        let once = std::sync::OnceLock::new();
        assert_eq!(fresh_scratch_dir(&once, prefix), dir);
        assert!(!dir.exists(), "an earlier process's files are gone");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("now"), "this process").unwrap();
        assert_eq!(fresh_scratch_dir(&once, prefix), dir);
        assert!(dir.join("now").exists(), "emptied only on the first use");
        std::fs::remove_dir_all(&dir).unwrap();

        let other = tempfile::tempdir().unwrap();
        let kept = other.path().join(format!("x-{}", std::process::id()));
        std::fs::create_dir_all(&kept).unwrap();
        let prefix = format!("{}/x", other.path().file_name().unwrap().to_string_lossy());
        fresh_scratch_dir(&std::sync::OnceLock::new(), &prefix);
        assert!(
            kept.is_dir(),
            "a folder not named <prefix>-<pid> in the temp folder stays"
        );
    }

    /// A link at the scratch folder is removed as a link: the folder it points at, and its files, stay.
    #[cfg(unix)]
    #[test]
    fn a_link_at_the_scratch_folder_is_removed_never_followed() {
        let prefix = "bir-test-scratch-link";
        let link = temp_dir().join(format!("{prefix}-{}", std::process::id()));
        let _ = std::fs::remove_file(&link);
        let target = tempfile::tempdir().unwrap();
        std::fs::write(target.path().join("kept"), "not ours").unwrap();
        std::os::unix::fs::symlink(target.path(), &link).unwrap();
        assert_eq!(fresh_scratch_dir(&std::sync::OnceLock::new(), prefix), link);
        assert!(
            std::fs::symlink_metadata(&link).is_err(),
            "the link is gone"
        );
        assert!(
            target.path().join("kept").is_file(),
            "its target is untouched"
        );
    }

    /// A junction at the scratch folder is removed as a link: the folder it points at, and its files, stay.
    #[cfg(windows)]
    #[test]
    fn a_junction_at_the_scratch_folder_is_removed_never_followed() {
        let prefix = "bir-test-scratch-junction";
        let link = temp_dir().join(format!("{prefix}-{}", std::process::id()));
        let _ = std::fs::remove_dir(&link);
        let target = tempfile::tempdir().unwrap();
        std::fs::write(target.path().join("kept"), "not ours").unwrap();
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(target.path())
            .output()
            .unwrap();
        assert!(made.status.success(), "mklink /J: {made:?}");
        assert!(is_reparse_point(&std::fs::symlink_metadata(&link).unwrap()));
        assert_eq!(fresh_scratch_dir(&std::sync::OnceLock::new(), prefix), link);
        assert!(
            std::fs::symlink_metadata(&link).is_err(),
            "the junction is gone"
        );
        assert!(
            target.path().join("kept").is_file(),
            "its target is untouched"
        );
    }

    #[cfg(target_os = "android")]
    #[test]
    fn android_keychain_is_disabled_even_when_the_switch_is_off() {
        temp_env::with_var(NO_KEYCHAIN_ENV, Some("0"), || {
            assert!(keychain_disabled());
        });
    }
}
