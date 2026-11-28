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
    match crate::db::single_room_database() {
        Some(named) => named
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from(".")),
        None => data_dir(),
    }
}

/// The folder [`prepare_app_storage`] excludes from backups on iOS and gives
/// file protection: the whole [`data_dir`]. Every new folder of the rooms
/// layout (`rooms/`, `backups/`, `previews/`, `device.db`) is made inside it
/// (Lead's review point 1), never beside it.
pub fn backup_excluded_dir() -> std::path::PathBuf {
    data_dir()
}

/// The print-preview hand-off folder: exactly the room layout's `previews/`.
/// In a single-room test or development run the room layout is beside the
/// scratch database that run named, so this can never be the installed app's
/// real data folder; in the ordinary layout it is the device folder's
/// `previews/`.
pub fn print_previews_dir() -> std::path::PathBuf {
    crate::rooms::Layout::here().previews_dir()
}

/// Run once at launch, before the database is opened. iOS excludes the data
/// folder from backups, gives it file protection "complete until first user
/// authentication" (files made in it take it too), and deletes the three
/// Keychain items one earlier build stored once (`bir_submit::ios_storage::
/// remove_old_items`; no Keychain call with `BUWIZ_NO_KEYCHAIN=1`).
///
/// Print previews are swept by [`prepare_print_previews`], separately: the
/// desktop host calls that only after its single-instance guard, so a second
/// launch cannot delete a file the first copy is writing.
pub fn prepare_app_storage() {
    #[cfg(target_os = "ios")]
    {
        let (warn, line) = ios::prepare_ios_storage();
        LAUNCH_STORAGE.record(warn, line);
        // The profile export's temporary copy is removed when the sheet answers; a crash leaves it — sweep those here.
        let exports = temp_dir().join("profile-exports");
        if exports.is_dir() {
            let _ = std::fs::remove_dir_all(&exports);
        }
    }
}

/// Whether the launch sweep may empty `previews`: only a path inside the app's own data directory. A dev
/// single-room run can put its scratch database in Documents, and emptying a "previews" folder there deletes
/// what is not ours.
fn previews_sweep_allowed(previews: &std::path::Path, data: &std::path::Path) -> bool {
    previews.starts_with(data)
}

/// Sweep every print-preview hand-off location once at launch. The current
/// private folder is emptied; the retired `$TMPDIR/bir-*-preview.html` and
/// Android `data_dir()/tmp/bir-*-preview.html` files are removed by exact
/// filename pattern. Nothing is traversed through a link and a failure is
/// recorded for the launch log, never fatal.
pub fn prepare_print_previews() {
    static SWEPT: std::sync::Once = std::sync::Once::new();
    SWEPT.call_once(|| {
        #[cfg(test)]
        PRINT_SWEEP_RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut errors = Vec::new();
        // Never sweep outside the app's own data directory: in a dev single-room run the scratch database can sit in
        // Documents, and emptying a "previews" folder there deletes what is not ours. The previews still write there;
        // they are just never swept.
        if previews_sweep_allowed(&print_previews_dir(), &data_dir()) {
            if let Err(error) = sweep_preview_html_in(print_previews_dir()) {
                errors.push(format!("print previews not swept at launch: {error}"));
            }
        }
        if let Err(error) = sweep_retired_preview_files_in(std::env::temp_dir(), data_dir()) {
            errors.push(format!(
                "retired print previews not swept at launch: {error}"
            ));
        }
        if !errors.is_empty() {
            LAUNCH_STORAGE.record(true, errors.join("; "));
        }
    });
}

/// Empty `folder` without following a link out of it. The folder itself stays
/// when it is a real folder; a link or ordinary file at its name is removed as
/// that object, never traversed. Real folders inside it are removed
/// recursively after their own contents have been checked.
fn sweep_preview_html_in(folder: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    let folder = folder.as_ref();
    let Ok(meta) = std::fs::symlink_metadata(folder) else {
        return Ok(());
    };
    if meta.file_type().is_symlink() || is_reparse_point(&meta) || !meta.is_dir() {
        return remove_preview_object(folder, &meta);
    }
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let child_meta = std::fs::symlink_metadata(entry.path())?;
        remove_preview_object(&entry.path(), &child_meta)?;
    }
    Ok(())
}

/// Remove exact `bir-*-preview.html` matches in the retired OS temp folder
/// and Android's `data_dir()/tmp`. A matching symlink is removed as a symlink;
/// a matching directory is left alone because these locations only ever held
/// files.
fn sweep_retired_preview_files_in(
    temp_folder: impl AsRef<std::path::Path>,
    app_data: impl AsRef<std::path::Path>,
) -> std::io::Result<()> {
    let mut failures: Vec<String> = Vec::new();
    for folder in [
        temp_folder.as_ref().to_path_buf(),
        app_data.as_ref().join("tmp"),
    ] {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    failures.push(format!("{} cannot be listed: {error}", folder.display()));
                    continue;
                }
            };
            let retired = entry
                .file_name()
                .to_str()
                .is_some_and(is_retired_preview_file_name);
            if !retired {
                continue;
            }
            let meta = match std::fs::symlink_metadata(entry.path()) {
                Ok(meta) => meta,
                Err(error) => {
                    failures.push(format!(
                        "{} cannot be inspected: {error}",
                        entry.path().display()
                    ));
                    continue;
                }
            };
            if meta.is_dir() && !meta.file_type().is_symlink() && !is_reparse_point(&meta) {
                continue;
            }
            if let Err(error) =
                std::fs::remove_file(entry.path()).or_else(|_| std::fs::remove_dir(entry.path()))
            {
                failures.push(format!(
                    "{} cannot be removed: {error}",
                    entry.path().display()
                ));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(std::io::Error::other(failures.join("; ")))
    }
}

fn is_retired_preview_file_name(name: &str) -> bool {
    name.strip_prefix("bir-")
        .and_then(|rest| rest.strip_suffix("-preview.html"))
        .is_some_and(|middle| !middle.is_empty())
}

fn remove_preview_object(path: &std::path::Path, meta: &std::fs::Metadata) -> std::io::Result<()> {
    if meta.file_type().is_symlink() || is_reparse_point(meta) {
        return std::fs::remove_file(path).or_else(|_| std::fs::remove_dir(path));
    }
    if !meta.is_dir() {
        return std::fs::remove_file(path);
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let child_meta = std::fs::symlink_metadata(entry.path())?;
        remove_preview_object(&entry.path(), &child_meta)?;
    }
    std::fs::remove_dir(path)
}

#[cfg(test)]
static PRINT_SWEEP_RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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

/// Launch lines made before logging exists, logged once when it does.
struct Pending {
    lines: std::sync::Mutex<Vec<(bool, String)>>,
}

impl Pending {
    const fn new() -> Self {
        Self {
            lines: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Keep `line` (`warn`: at warning level) until [`Pending::flush`]. Lines
    /// accumulate: a sweep warning must not be overwritten by the storage line.
    fn record(&self, warn: bool, line: String) {
        self.lines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push((warn, line));
    }

    /// Hand the kept lines to `emit` and forget them: at most once each.
    fn flush(&self, mut emit: impl FnMut(bool, &str)) {
        let taken = std::mem::take(
            &mut *self
                .lines
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        for (warn, line) in taken {
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

    /// F8: a previews folder outside the app's data directory is never swept — a dev scratch database in
    /// Documents must not have its sibling "previews" emptied; one inside the data directory is.
    #[test]
    fn the_preview_sweep_is_refused_outside_the_data_directory() {
        let scratch = tempfile::tempdir().unwrap();
        let data = scratch.path().join("data");
        let inside = data.join("previews");
        let outside = scratch.path().join("documents").join("previews");
        assert!(previews_sweep_allowed(&inside, &data));
        assert!(!previews_sweep_allowed(&outside, &data));
        assert!(!previews_sweep_allowed(
            std::path::Path::new("/Users/shared/Documents/previews"),
            &data
        ));
    }

    /// The one preview folder is the room layout's folder. In a single-room
    /// test it is beside that test's scratch database, never the installed
    /// app's data folder; in the ordinary multi-room layout it is the device
    /// folder's `previews/`.
    #[test]
    fn the_print_preview_folder_is_the_layouts_and_test_scratch_folder() {
        let scratch = tempfile::tempdir().unwrap();
        let database = scratch.path().join("room").join("bir_data.db");
        temp_env::with_var("BIR_DATABASE_PATH", None::<&str>, || {
            crate::db::capture_database_path_override().run(|| {
                crate::db::set_database_path_override(Some(database.clone()));
                assert_eq!(
                    print_previews_dir(),
                    scratch.path().join("room").join("previews")
                );
                crate::db::set_database_path_override(None);
            });
        });
    }

    /// Every test gets its own folder: the sweep itself is exercised against
    /// a folder named by the test, not the process-wide scratch data folder.
    #[test]
    fn the_preview_sweep_uses_only_the_folder_it_is_given() {
        let scratch = tempfile::tempdir().unwrap();
        let previews = scratch.path().join("previews");
        std::fs::create_dir_all(&previews).unwrap();
        let leftover = previews.join("leftover-preview.html");
        std::fs::write(&leftover, "<html>filled return</html>").unwrap();
        sweep_preview_html_in(&previews).unwrap();
        assert!(!leftover.exists(), "the launch sweep removes it");
    }

    /// The retired temp-folder locations are swept once by their exact
    /// filename pattern. Other temp files stay, and a match is removed as the
    /// object it is, never traversed.
    #[test]
    fn the_retired_temp_preview_locations_are_swept_by_exact_pattern() {
        let scratch = tempfile::tempdir().unwrap();
        let old_temp = scratch.path().join("tmp");
        let data = scratch.path().join("data");
        std::fs::create_dir_all(&old_temp).unwrap();
        std::fs::create_dir_all(data.join("tmp")).unwrap();
        let retired = old_temp.join("bir-old-preview.html");
        let android_retired = data.join("tmp").join("bir-android-preview.html");
        let keep = old_temp.join("bir-other.txt");
        let keep_prefix = old_temp.join("xbir-old-preview.html");
        std::fs::write(&retired, "filled").unwrap();
        std::fs::write(&android_retired, "filled").unwrap();
        std::fs::write(&keep, "keep").unwrap();
        std::fs::write(&keep_prefix, "keep").unwrap();
        sweep_retired_preview_files_in(&old_temp, &data).unwrap();
        assert!(!retired.exists());
        assert!(!android_retired.exists());
        assert!(keep.exists());
        assert!(keep_prefix.exists());
    }

    /// An undeletable retired file is one failure, not the end of the sweep:
    /// later retired files still go and the failure is reported once by the caller.
    #[cfg(unix)]
    #[test]
    fn the_retired_sweep_continues_after_a_file_it_cannot_remove() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = tempfile::tempdir().unwrap();
        let old_temp = scratch.path().join("tmp");
        let data = scratch.path().join("data");
        std::fs::create_dir_all(&old_temp).unwrap();
        std::fs::create_dir_all(data.join("tmp")).unwrap();
        let stuck = old_temp.join("bir-stuck-preview.html");
        let later = data.join("tmp").join("bir-later-preview.html");
        std::fs::write(&stuck, "filled").unwrap();
        std::fs::write(&later, "filled").unwrap();
        std::fs::set_permissions(&old_temp, std::fs::Permissions::from_mode(0o555)).unwrap();
        assert!(sweep_retired_preview_files_in(&old_temp, &data).is_err());
        assert!(stuck.exists());
        assert!(!later.exists());
        std::fs::set_permissions(&old_temp, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// The public launch sweep is process-once: repeated Door calls do not
    /// sweep (or repeat its warning) again.
    #[test]
    fn two_launch_sweep_calls_run_one_sweep() {
        let scratch = tempfile::tempdir().unwrap();
        let database = scratch.path().join("room").join("bir_data.db");
        crate::db::capture_database_path_override().run(|| {
            crate::db::set_database_path_override(Some(database));
            prepare_print_previews();
            prepare_print_previews();
            crate::db::set_database_path_override(None);
        });
        assert_eq!(
            PRINT_SWEEP_RUNS.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    /// A sweep failure and the iOS storage line are both kept, in the order
    /// they happened, instead of one overwriting the other.
    #[test]
    fn pending_launch_storage_lines_do_not_overwrite_each_other() {
        let pending = Pending::new();
        let mut logged = Vec::new();
        pending.record(true, "Print previews not swept at launch: denied".into());
        pending.record(false, "App storage at launch: protected".into());
        pending.flush(|warn, line| logged.push((warn, line.to_owned())));
        assert_eq!(
            logged,
            [
                (
                    true,
                    "Print previews not swept at launch: denied".to_owned()
                ),
                (false, "App storage at launch: protected".to_owned())
            ]
        );
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
