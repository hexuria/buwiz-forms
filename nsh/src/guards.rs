//! The two guards that stand between the TSX screens and the owner's data or the BIR.
//!
//! * The **send guard** ([`send_guard`]) runs once, before the first database open and before the
//!   submission worker starts. It forces a dry run, drops the live acknowledgement, and decides whether the
//!   worker may start at all. The one way out of a dry run is the mock BIR server on loopback.
//! * The **database guard** ([`database_refusal`]) refuses to open a database inside the owner's data
//!   folders, by name and by device and inode. It is checked before every host call, so a lazy open can
//!   never get around it. The folders hang off the home folder of the account's real uid, not `$HOME`.
//!
//! Both are plain functions over the environment and a list of folders, so their tests use temp folders and
//! never touch a real path. Contract: `docs/plans/nsh-tsx-migration.md`, "The door".
//!
//! **Platforms.** On Unix (macOS, Linux) the database guard is complete: the account's real home folder from
//! the password database, and a folder or file known by device and inode. Windows has neither here (std has no
//! inode, and the profile folder std finds starts from the `USERPROFILE` variable, which a caller can change), and
//! nobody has run this crate on Windows. So there the guard opens a database only when the caller names it
//! with `BIR_DATABASE_PATH` ([`unverified_refusal`]); the by-name check still applies to that path. A Windows
//! owner-folder lookup and same-file check are not written: see `docs/platforms/nsh.md`.
//!
//! **Environment.** The send guard is the only code here that changes the process environment (two
//! variables). It runs once, from the door's process setup, which the native module's registration calls
//! (see `door::process`); nothing else in this crate sets or removes a variable.

use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use bir_core::dev_mode::SubmissionMode;

/// What the send guard decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendMode {
    /// `BIR_SFTP_DRY_RUN=1`: a queued return is "sent" to a folder on this computer, nothing leaves it.
    DryRun,
    /// The mock BIR server (`just mock-bir`) is listening on loopback and the transport sends there, and
    /// nowhere else: its host is `127.0.0.1` or `::1`.
    Mock { host: String, port: u16 },
    /// Sending is not a dry run and not the mock: the submission worker does not start. Why.
    Refused(String),
}

impl SendMode {
    /// Whether the submission worker may start.
    pub fn worker_may_start(&self) -> bool {
        !matches!(self, SendMode::Refused(_))
    }

    /// The word TSX sees: `dry_run`, `mock` or `refused`.
    pub fn label(&self) -> &'static str {
        match self {
            SendMode::DryRun => "dry_run",
            SendMode::Mock { .. } => "mock",
            SendMode::Refused(_) => "refused",
        }
    }

    /// A sentence for the status, when there is something to say.
    pub fn note(&self) -> Option<String> {
        match self {
            SendMode::DryRun => None,
            SendMode::Mock { host, port } => Some(format!(
                "Sending to the mock BIR server at {host}:{port}. Nothing reaches the BIR."
            )),
            SendMode::Refused(reason) => Some(reason.clone()),
        }
    }
}

/// How long the mock's port gets to answer.
const LISTEN_TIMEOUT: Duration = Duration::from_millis(400);

/// Set `BIR_SFTP_DRY_RUN=1`, remove `BIR_SFTP_LIVE`, then check that the transport really is a dry run and
/// that the Dev Mode panel does not point at the test server. Mock mode is the only exception: a listening
/// loopback `BIR_SFTP_HOST` leaves the environment as the `mock-bir-app` recipe made it.
///
/// Call it once, before the database opens (the process boot does).
pub fn send_guard() -> SendMode {
    send_guard_with(port_is_listening)
}

/// [`send_guard`] with the "is anything listening there" probe given, for tests.
pub(crate) fn send_guard_with(listening: impl Fn(&str, u16) -> bool) -> SendMode {
    // SAFETY: the guard runs once, from `door::process` (called when the native module registers, before any
    // thread of ours or of NSH's app exists; see there), before the database and the worker exist; the
    // environment is only read by the transport afterwards. A test calls it directly under `temp_env`.
    unsafe { std::env::remove_var("BIR_SFTP_LIVE") };
    let mock = mock_target(&listening);
    if mock.is_none() {
        // SAFETY: as above.
        unsafe { std::env::set_var("BIR_SFTP_DRY_RUN", "1") };
    }
    verdict(
        mock,
        bir_core::transport::is_sftp_dry_run(),
        bir_core::dev_mode::panel_submission_mode(),
    )
}

/// The decision, from what was observed. Pure, so every branch is tested.
pub(crate) fn verdict(
    mock: Option<(String, u16)>,
    dry_run: bool,
    panel: Option<SubmissionMode>,
) -> SendMode {
    if panel == Some(SubmissionMode::TestServer) {
        return SendMode::Refused(
            "The Dev Mode panel points submissions at the test server; the NSH app only sends to a dry run or the local mock."
                .into(),
        );
    }
    if dry_run {
        return SendMode::DryRun;
    }
    match mock {
        Some((host, port)) => SendMode::Mock { host, port },
        None => SendMode::Refused(
            "Sending is not a dry run, so the submission worker was not started.".into(),
        ),
    }
}

/// `BIR_SFTP_HOST` when it is `127.0.0.1` or `::1` and its port (`BIR_SFTP_PORT`, 22 when unset) answers.
fn mock_target(listening: &impl Fn(&str, u16) -> bool) -> Option<(String, u16)> {
    let host = std::env::var("BIR_SFTP_HOST").ok()?;
    let host = host.trim();
    if host != "127.0.0.1" && host != "::1" {
        return None;
    }
    let port = match std::env::var("BIR_SFTP_PORT") {
        Ok(raw) if !raw.trim().is_empty() => raw.trim().parse::<u16>().ok()?,
        _ => 22,
    };
    listening(host, port).then(|| (host.to_owned(), port))
}

fn port_is_listening(host: &str, port: u16) -> bool {
    let Ok(ip) = host.parse::<IpAddr>() else {
        return false;
    };
    TcpStream::connect_timeout(&SocketAddr::new(ip, port), LISTEN_TIMEOUT).is_ok()
}

/// The folders the door never opens a database in, under `home`.
pub fn protected_folders(home: &Path) -> [PathBuf; 3] {
    [
        home.join("Library/Group Containers/group.dev.goldcoders.bir"),
        home.join("BuwizNsh/data"),
        home.join("BuwizNsh/migration"),
    ]
}

/// The home folder of the user this process runs as, from the password database (the real uid), never
/// from `$HOME`: a process started with `HOME=/tmp/x` still protects the owner's real folders. `None` when
/// the account has no entry. On Windows, where there is no password database, the profile folder std knows: that
/// starts from `USERPROFILE`, so it is a best effort for the by-name check and is not trusted (see
/// [`unverified_refusal`], which is why Windows opens only a database the caller names).
pub fn account_home() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        use std::ffi::{CStr, OsStr};
        use std::os::unix::ffi::OsStrExt;

        // SAFETY: `getpwuid_r` fills `entry` and the strings it points to inside `buffer`, both of which
        // outlive the reads below; the result is copied out before they go away.
        unsafe {
            let mut entry: libc::passwd = std::mem::zeroed();
            let mut found: *mut libc::passwd = std::ptr::null_mut();
            let mut buffer = vec![0u8; 16 * 1024];
            let status = libc::getpwuid_r(
                libc::getuid(),
                &mut entry,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut found,
            );
            if status != 0 || found.is_null() || entry.pw_dir.is_null() {
                return None;
            }
            let dir = CStr::from_ptr(entry.pw_dir).to_bytes();
            (!dir.is_empty()).then(|| PathBuf::from(OsStr::from_bytes(dir)))
        }
    }
    #[cfg(not(unix))]
    {
        std::env::home_dir()
    }
}

/// The owner's protected folders, under the account's real home folder. `None` when that cannot be found.
fn protected_folders_here() -> Option<Vec<PathBuf>> {
    #[cfg(test)]
    if let Some(folders) = TEST_PROTECTED.with(|slot| slot.borrow().clone()) {
        return Some(folders);
    }
    account_home().map(|home| protected_folders(&home).to_vec())
}

#[cfg(test)]
thread_local! {
    /// Test seam: the protected folders of this thread, in place of the real account's (a test never
    /// reads the owner's real folders).
    static TEST_PROTECTED: std::cell::RefCell<Option<Vec<PathBuf>>> = const { std::cell::RefCell::new(None) };
}

/// Run `body` with `home` standing in for the account's home folder, on this thread.
#[cfg(test)]
pub(crate) fn with_home_for_test<R>(home: &Path, body: impl FnOnce() -> R) -> R {
    struct Restore(Option<Vec<PathBuf>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_PROTECTED.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let before = TEST_PROTECTED.with(|slot| slot.replace(Some(protected_folders(home).to_vec())));
    let _restore = Restore(before);
    body()
}

/// Why the door must not open `path` as the database, or `None` when it may. `protected` are the folders
/// that must stay untouched; they need not exist.
///
/// Two checks, either of which refuses:
///
/// * **By name.** `path` is made absolute and resolved through symlinks (as far as it exists, then the rest
///   by name), so `..` and links cannot reach a protected folder; names compare case-insensitively where
///   the file system usually is (macOS, Windows).
/// * **By identity** (Unix). A protected folder that exists is known by its device and inode, which no
///   spelling changes: macOS reaches the same folder as `/Users/u/x` and `/System/Volumes/Data/Users/u/x`
///   (a firmlink, not a symlink), a case-insensitive volume answers to any case, a bind mount to its other
///   path. So `path` is refused when any ancestor of it that exists IS a protected folder, and, when it is
///   an existing file with more than one link, when it is the same file as one directly inside a protected
///   folder (a hard link).
pub fn database_refusal(path: &Path, protected: &[PathBuf]) -> Option<String> {
    let canonical = canonical_target(path);
    let folders: Vec<PathBuf> = protected
        .iter()
        .map(|folder| canonical_target(folder))
        .collect();
    let refuse = |how: String| {
        Some(format!(
            "Refusing to open {}: it is {how}, the owner's data. Copy a database to ~/BuwizNsh/agents/<track>.db and point BIR_DATABASE_PATH there.",
            canonical.display()
        ))
    };
    for folder in &folders {
        if inside(&canonical, folder) {
            return refuse(format!("inside {}", folder.display()));
        }
    }
    let identified: Vec<(Identity, &PathBuf)> = folders
        .iter()
        .filter(|folder| folder.is_dir())
        .filter_map(|folder| identity(folder).map(|id| (id, folder)))
        .collect();
    if !identified.is_empty() {
        for ancestor in canonical.ancestors() {
            if let Some(id) = identity(ancestor)
                && let Some((_, folder)) = identified.iter().find(|(known, _)| *known == id)
            {
                return refuse(format!(
                    "inside {} (the same folder by device and inode, under another name)",
                    folder.display()
                ));
            }
        }
        if let Some(file) = hard_linked_file_identity(&canonical) {
            for (_, folder) in &identified {
                let Ok(entries) = std::fs::read_dir(folder) else {
                    continue;
                };
                if entries
                    .flatten()
                    .any(|entry| identity(&entry.path()) == Some(file))
                {
                    return refuse(format!("a hard link to a file in {}", folder.display()));
                }
            }
        }
    }
    None
}

/// [`database_refusal`] for the owner's real folders. With no home folder to name them, the answer is a
/// refusal.
pub fn database_refusal_here(path: &Path) -> Option<String> {
    // A test that names its own "home" (the seam) checks the by-name rules on any system.
    #[cfg(test)]
    let seamed = TEST_PROTECTED.with(|slot| slot.borrow().is_some());
    #[cfg(not(test))]
    let seamed = false;
    if !seamed
        && let Some(why) = unverified_refusal(
            cfg!(unix),
            path,
            std::env::var_os(DATABASE_PATH_ENV),
        )
    {
        return Some(why);
    }
    refusal_with(path, protected_folders_here())
}

/// The variable that names the database file (bir-core reads it as well).
const DATABASE_PATH_ENV: &str = "BIR_DATABASE_PATH";

/// Where the guard cannot do what it promises (`can_verify` is false: Windows, anything without Unix's password
/// database and inodes), it opens no database the caller has not named: `path` must be the file `BIR_DATABASE_PATH`
/// names (`named`). The default data folder is refused there, whoever asks. `None` when `can_verify` or when the
/// caller named this very file. Pure, so every branch runs on any system.
pub(crate) fn unverified_refusal(
    can_verify: bool,
    path: &Path,
    named: Option<std::ffi::OsString>,
) -> Option<String> {
    if can_verify {
        return None;
    }
    let named = named.filter(|value| !value.is_empty());
    if named.is_some_and(|value| canonical_target(Path::new(&value)) == canonical_target(path)) {
        return None;
    }
    Some(format!(
        "Refusing to open {}: on this system the guard cannot tell which folders are the owner's data (no password database, no file identity), so it opens only a database you name. Set BIR_DATABASE_PATH to the file.",
        path.display()
    ))
}

/// [`database_refusal`] over the folders found, or the refusal for there being none to find.
fn refusal_with(path: &Path, protected: Option<Vec<PathBuf>>) -> Option<String> {
    let Some(protected) = protected else {
        return Some(
            "Cannot tell where the owner's data folders are (no home folder), so no database is opened."
                .into(),
        );
    };
    database_refusal(path, &protected)
}

/// A file or folder as the file system knows it, whatever the path that reached it: (device, inode).
type Identity = (u64, u64);

#[cfg(unix)]
fn identity(path: &Path) -> Option<Identity> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .ok()
        .map(|meta| (meta.dev(), meta.ino()))
}

/// Windows has no device and inode in std; there the guard is by name only.
#[cfg(not(unix))]
fn identity(_path: &Path) -> Option<Identity> {
    None
}

/// The identity of `path` when it is an existing regular file with more than one link.
#[cfg(unix)]
fn hard_linked_file_identity(path: &Path) -> Option<Identity> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).ok()?;
    (meta.is_file() && meta.nlink() > 1).then(|| (meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
fn hard_linked_file_identity(_path: &Path) -> Option<Identity> {
    None
}

fn inside(path: &Path, folder: &Path) -> bool {
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        // These file systems are case-insensitive by default.
        let lower = |p: &Path| PathBuf::from(p.to_string_lossy().to_lowercase());
        lower(path).starts_with(lower(folder))
    } else {
        path.starts_with(folder)
    }
}

/// An absolute, symlink-free form of `path` that need not exist, resolved the way the OS would open it:
/// component by component, a link followed at the point it is met, `..` taken from the real parent.
fn canonical_target(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    resolve(&absolute, 0)
}

/// More links than any real path has: a loop stops here and keeps what it has.
const MAX_LINKS: u32 = 40;

fn resolve(path: &Path, links: u32) -> PathBuf {
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => resolved.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => {
                let candidate = resolved.join(name);
                match std::fs::read_link(&candidate) {
                    // A link, dangling or not: opening it would create or use what it points at.
                    Ok(target) if links < MAX_LINKS => {
                        let target = if target.is_absolute() {
                            target
                        } else {
                            resolved.join(target)
                        };
                        resolved = resolve(&target, links + 1);
                    }
                    _ => resolved = candidate,
                }
            }
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::TcpListener;

    use super::*;

    /// Every variable the send guard reads or writes, so a test runs with exactly the ones it names.
    fn sending_env<R>(set: &[(&str, &str)], body: impl FnOnce() -> R) -> R {
        let names = [
            "BIR_SFTP_DRY_RUN",
            "BIR_SFTP_LIVE",
            "BIR_SFTP_HOST",
            "BIR_SFTP_PORT",
            "EBIR_TEST_ENV",
        ];
        let vars: Vec<(&str, Option<&str>)> = names
            .iter()
            .map(|name| {
                (
                    *name,
                    set.iter()
                        .find(|(set, _)| set == name)
                        .map(|(_, value)| *value),
                )
            })
            .collect();
        temp_env::with_vars(vars, body)
    }

    fn live() -> Option<String> {
        std::env::var("BIR_SFTP_LIVE").ok()
    }

    fn dry_run() -> Option<String> {
        std::env::var("BIR_SFTP_DRY_RUN").ok()
    }

    #[test]
    fn a_dry_run_is_forced_and_the_live_acknowledgement_removed() {
        sending_env(&[("BIR_SFTP_LIVE", "1")], || {
            assert_eq!(send_guard(), SendMode::DryRun);
            assert_eq!(dry_run().as_deref(), Some("1"));
            assert_eq!(live(), None);
            assert!(bir_core::transport::is_sftp_dry_run());
        });
    }

    #[test]
    fn a_live_ack_does_not_survive_a_lab_host_that_is_not_the_mock() {
        sending_env(
            &[
                ("BIR_SFTP_LIVE", "1"),
                ("BIR_SFTP_HOST", "sftp.example.test"),
            ],
            || {
                // Even a host that answers is not loopback: it is never the mock.
                assert_eq!(send_guard_with(|_, _| true), SendMode::DryRun);
                assert_eq!(dry_run().as_deref(), Some("1"));
                assert_eq!(live(), None);
            },
        );
    }

    #[test]
    fn a_listening_loopback_host_is_the_mock_and_the_environment_is_left_alone() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("address").port().to_string();
        sending_env(
            &[
                ("BIR_SFTP_LIVE", "1"),
                ("BIR_SFTP_HOST", "127.0.0.1"),
                ("BIR_SFTP_PORT", port.as_str()),
            ],
            || {
                let mode = send_guard();
                assert_eq!(
                    mode,
                    SendMode::Mock {
                        host: "127.0.0.1".into(),
                        port: port.parse().unwrap()
                    }
                );
                assert!(mode.worker_may_start());
                // The mock recipe's environment: no forced dry run, and still no live ack.
                assert_eq!(dry_run(), None);
                assert_eq!(live(), None);
                assert!(!bir_core::transport::is_sftp_dry_run());
            },
        );
    }

    #[test]
    fn the_mock_works_on_the_ipv6_loopback_too() {
        sending_env(
            &[("BIR_SFTP_HOST", "::1"), ("BIR_SFTP_PORT", "2222")],
            || {
                let asked = std::cell::RefCell::new(Vec::new());
                let mode = send_guard_with(|host, port| {
                    asked.borrow_mut().push((host.to_owned(), port));
                    true
                });
                assert_eq!(
                    mode,
                    SendMode::Mock {
                        host: "::1".into(),
                        port: 2222
                    }
                );
                assert_eq!(asked.into_inner(), vec![("::1".to_owned(), 2222)]);
            },
        );
    }

    #[test]
    fn a_loopback_host_with_nothing_listening_is_a_dry_run() {
        let free = {
            let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
            listener.local_addr().expect("address").port().to_string()
        };
        sending_env(
            &[
                ("BIR_SFTP_HOST", "127.0.0.1"),
                ("BIR_SFTP_PORT", free.as_str()),
            ],
            || {
                assert_eq!(send_guard(), SendMode::DryRun);
                assert_eq!(dry_run().as_deref(), Some("1"));
            },
        );
    }

    #[test]
    fn a_test_environment_stays_a_dry_run_even_with_the_mock_listening() {
        sending_env(
            &[("EBIR_TEST_ENV", "1"), ("BIR_SFTP_HOST", "127.0.0.1")],
            || {
                // The host is the mock and it answers, but the transport is a dry run, and the verdict says so.
                assert_eq!(send_guard_with(|_, _| true), SendMode::DryRun);
                assert_eq!(dry_run(), None);
            },
        );
    }

    #[test]
    fn a_bad_mock_port_is_not_the_mock() {
        sending_env(
            &[
                ("BIR_SFTP_HOST", "127.0.0.1"),
                ("BIR_SFTP_PORT", "not-a-port"),
            ],
            || assert_eq!(send_guard_with(|_, _| true), SendMode::DryRun),
        );
    }

    #[test]
    fn the_verdict_refuses_what_is_not_a_dry_run_or_the_mock() {
        use SubmissionMode::{DryRun, TestServer};
        // The test server in the Dev Mode panel is refused whatever else is true.
        for mock in [None, Some(("127.0.0.1".to_owned(), 22))] {
            for dry in [false, true] {
                let mode = verdict(mock.clone(), dry, Some(TestServer));
                assert!(matches!(mode, SendMode::Refused(_)), "{mock:?} {dry}");
                assert!(!mode.worker_may_start());
                assert_eq!(mode.label(), "refused");
                assert!(mode.note().is_some());
            }
        }
        // Not a dry run, and not the mock: refused.
        let refused = verdict(None, false, None);
        assert!(matches!(refused, SendMode::Refused(_)));
        assert!(!refused.worker_may_start());
        // A dry run, with or without the panel's dry-run mode.
        assert_eq!(verdict(None, true, None), SendMode::DryRun);
        assert_eq!(verdict(None, true, Some(DryRun)), SendMode::DryRun);
        assert!(SendMode::DryRun.worker_may_start());
        // The mock counts only while the transport really sends to it.
        let mock = Some(("::1".to_owned(), 2222));
        assert_eq!(
            verdict(mock.clone(), false, None),
            SendMode::Mock {
                host: "::1".into(),
                port: 2222
            }
        );
        assert_eq!(verdict(mock, true, None), SendMode::DryRun);
    }

    /// A made-up home folder with the owner's three data folders under it.
    struct Home {
        root: tempfile::TempDir,
    }

    impl Home {
        fn new() -> Self {
            Home {
                root: tempfile::tempdir().expect("temp home"),
            }
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.path().join(relative)
        }

        fn make(&self, relative: &str) -> PathBuf {
            let path = self.path(relative);
            fs::create_dir_all(&path).expect("make folder");
            path
        }

        fn refusal(&self, path: &Path) -> Option<String> {
            database_refusal(path, &protected_folders(self.root.path()))
        }
    }

    const GROUP: &str = "Library/Group Containers/group.dev.goldcoders.bir";

    #[test]
    fn a_database_inside_each_protected_folder_is_refused() {
        let home = Home::new();
        for folder in [GROUP, "BuwizNsh/data", "BuwizNsh/migration"] {
            // Folder missing: still refused, by name.
            let file = home.path(&format!("{folder}/bir_data.db"));
            let why = home
                .refusal(&file)
                .unwrap_or_else(|| panic!("{folder} not refused"));
            assert!(why.contains("Refusing to open"), "{why}");
            // The folder itself, a deeper path, and a file that exists.
            home.make(folder);
            assert!(
                home.refusal(&home.path(folder)).is_some(),
                "{folder} itself"
            );
            assert!(
                home.refusal(&home.path(&format!("{folder}/deeper/still/x.db")))
                    .is_some(),
                "{folder} deeper"
            );
            fs::write(&file, b"x").expect("write");
            assert!(home.refusal(&file).is_some(), "{folder} existing file");
        }
        // Nothing was created or touched by asking.
        assert!(!home.path("BuwizNsh/data/deeper").exists());
    }

    #[test]
    fn other_paths_are_allowed() {
        let home = Home::new();
        home.make("BuwizNsh/data");
        assert_eq!(home.refusal(&home.path("BuwizNsh/agents/door.db")), None);
        home.make("BuwizNsh/agents");
        assert_eq!(home.refusal(&home.path("BuwizNsh/agents/door.db")), None);
        // A name that only starts like a protected folder is not inside it.
        assert_eq!(
            home.refusal(&home.path("BuwizNsh/data-copy/bir_data.db")),
            None
        );
        assert_eq!(
            home.refusal(&home.path("BuwizNsh/migrations/bir_data.db")),
            None
        );
        assert_eq!(
            home.refusal(&home.path("Library/Group Containers/group.dev.goldcoders.bir.nsh/x.db")),
            None
        );
        assert_eq!(
            home.refusal(
                &home.path("Library/Application Support/dev.goldcoders.bir.nsh/bir_data.db")
            ),
            None
        );
        // Another temp folder entirely.
        let elsewhere = tempfile::tempdir().expect("temp");
        assert_eq!(home.refusal(&elsewhere.path().join("bir_data.db")), None);
    }

    #[test]
    fn dot_dot_cannot_reach_a_protected_folder() {
        let home = Home::new();
        let sneaky = home.path("BuwizNsh/agents/../data/bir_data.db");
        assert!(home.refusal(&sneaky).is_some(), "agents missing");
        home.make("BuwizNsh/agents");
        assert!(home.refusal(&sneaky).is_some(), "agents there");
        let deep = home.path("BuwizNsh/agents/x/y/../../../migration/z.db");
        assert!(home.refusal(&deep).is_some(), "deep");
        // And a path that goes up and out of a protected folder is allowed again.
        home.make("BuwizNsh/data");
        assert_eq!(
            home.refusal(&home.path("BuwizNsh/data/../agents/door.db")),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_cannot_reach_a_protected_folder() {
        use std::os::unix::fs::symlink;
        let home = Home::new();
        let data = home.make("BuwizNsh/data");
        let agents = home.make("BuwizNsh/agents");
        // A link to the folder, and a file beyond it.
        symlink(&data, agents.join("link")).expect("link");
        assert!(
            home.refusal(&agents.join("link/bir_data.db")).is_some(),
            "link to the folder"
        );
        assert!(
            home.refusal(&agents.join("link")).is_some(),
            "the link itself"
        );
        // A dangling link to a file that would be created there.
        symlink(data.join("new.db"), agents.join("dangling.db")).expect("dangling");
        assert!(
            home.refusal(&agents.join("dangling.db")).is_some(),
            "dangling link"
        );
        // A relative link, and a link in the middle followed by "..".
        symlink("../data", agents.join("relative")).expect("relative");
        assert!(
            home.refusal(&agents.join("relative/x.db")).is_some(),
            "relative link"
        );
        let sub = home.make("BuwizNsh/data/sub");
        symlink(&sub, agents.join("deep")).expect("deep");
        assert!(
            home.refusal(&agents.join("deep/../x.db")).is_some(),
            "link then .."
        );
        // A link that leads somewhere harmless is harmless.
        let harmless = home.make("scratch");
        symlink(&harmless, agents.join("ok")).expect("ok");
        assert_eq!(home.refusal(&agents.join("ok/door.db")), None);
        // A loop does not hang the guard.
        symlink(agents.join("b"), agents.join("a")).expect("a");
        symlink(agents.join("a"), agents.join("b")).expect("b");
        assert_eq!(home.refusal(&agents.join("a/x.db")), None);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn a_different_case_does_not_get_past_the_guard() {
        let home = Home::new();
        assert!(
            home.refusal(&home.path("buwiznsh/DATA/bir_data.db"))
                .is_some()
        );
        assert!(
            home.refusal(&home.path("LIBRARY/group containers/GROUP.dev.goldcoders.bir/x.db"))
                .is_some()
        );
    }

    #[test]
    fn with_no_home_nothing_is_opened() {
        let why = refusal_with(Path::new("/somewhere/else.db"), None).expect("refused");
        assert!(why.contains("no home folder"), "{why}");
    }

    /// A folder tree like the owner's, for the identity checks: the three protected folders exist, with a
    /// database in each, plus a folder that is not protected. (Only the Unix tests use it: Windows has no identity checks.)
    #[cfg(unix)]
    struct Owner {
        home: Home,
    }

    #[cfg(unix)]
    impl Owner {
        fn new() -> Self {
            let home = Home::new();
            for folder in [
                GROUP,
                "BuwizNsh/data",
                "BuwizNsh/migration",
                "BuwizNsh/agents",
            ] {
                home.make(folder);
            }
            fs::write(home.path("BuwizNsh/data/bir_data.db"), b"owner data")
                .expect("owner database");
            Owner { home }
        }

        /// The temp home as the file system names it (macOS puts /var behind /private).
        fn real(&self, relative: &str) -> PathBuf {
            self.home
                .root
                .path()
                .canonicalize()
                .expect("canonical home")
                .join(relative)
        }
    }

    /// macOS reaches every folder of the data volume a second way, `/System/Volumes/Data/...`: a firmlink,
    /// which is not a symlink, so resolving links does not see it. The same file by that name is the same
    /// file. (Before the identity check, this spelling got through.)
    #[cfg(target_os = "macos")]
    #[test]
    fn the_firmlink_spelling_of_a_protected_folder_is_refused() {
        let data_volume = Path::new("/System/Volumes/Data");
        if !data_volume.is_dir() {
            return; // macOS before 10.15 has no firmlinks
        }
        let owner = Owner::new();
        let firm = |relative: &str| {
            let real = owner.real(relative);
            PathBuf::from(format!("{}{}", data_volume.display(), real.display()))
        };
        let spelled = firm("BuwizNsh/data/bir_data.db");
        assert_eq!(
            identity(&spelled),
            identity(&owner.real("BuwizNsh/data/bir_data.db")),
            "the firmlink reaches the same file (the premise of this test)"
        );
        for folder in [GROUP, "BuwizNsh/data", "BuwizNsh/migration"] {
            for file in ["bir_data.db", "new/deeper.db"] {
                let candidate = firm(&format!("{folder}/{file}"));
                let why = owner
                    .home
                    .refusal(&candidate)
                    .unwrap_or_else(|| panic!("{} got through", candidate.display()));
                assert!(why.contains("Refusing to open"), "{why}");
            }
        }
        // The folder itself, and a folder beside the protected ones, by the same spelling.
        assert!(owner.home.refusal(&firm("BuwizNsh/data")).is_some());
        assert_eq!(owner.home.refusal(&firm("BuwizNsh/agents/door.db")), None);
    }

    /// A hard link is another name for the same bytes: one made outside the protected folders is refused,
    /// one to some other file is not.
    #[cfg(unix)]
    #[test]
    fn a_hard_link_to_a_database_in_a_protected_folder_is_refused() {
        let owner = Owner::new();
        let agents = owner.real("BuwizNsh/agents");
        let alias = agents.join("alias.db");
        fs::hard_link(owner.real("BuwizNsh/data/bir_data.db"), &alias).expect("hard link");
        let why = owner.home.refusal(&alias).expect("hard link refused");
        assert!(why.contains("hard link"), "{why}");
        // Through a symlink to the alias too.
        std::os::unix::fs::symlink(&alias, agents.join("by-link.db")).expect("link");
        assert!(owner.home.refusal(&agents.join("by-link.db")).is_some());
        // Several names for a file that is not the owner's is fine.
        let mine = agents.join("mine.db");
        fs::write(&mine, b"mine").expect("write");
        fs::hard_link(&mine, agents.join("mine-too.db")).expect("hard link");
        assert_eq!(owner.home.refusal(&mine), None);
        assert_eq!(owner.home.refusal(&agents.join("mine-too.db")), None);
        // And a file with a single name, inside no protected folder, is fine.
        assert_eq!(owner.home.refusal(&agents.join("fresh.db")), None);
    }

    /// The `_nsh-db-guard` recipe of the root justfile is this guard in Python (for `just nsh-run` and
    /// `just nsh-shots`). Run its script, with the account's home swapped for a temp one, on the same
    /// folders: it must say what the door's guard says, and what each should say is stated.
    #[cfg(unix)]
    #[test]
    fn the_justfile_guard_says_what_the_door_guard_says() {
        use std::os::unix::fs::symlink;
        use std::process::Command;

        let justfile =
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../justfile"))
                .expect("the root justfile");
        let recipe = justfile
            .split("_nsh-db-guard db:")
            .nth(1)
            .expect("the _nsh-db-guard recipe");
        let script = recipe
            .split("<<'PY'\n")
            .nth(1)
            .expect("the recipe's python script")
            .lines()
            .take_while(|line| line.trim() != "PY")
            .map(|line| line.strip_prefix("    ").unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");
        const HOME_LINE: &str = "home = pwd.getpwuid(os.getuid()).pw_dir";
        assert!(
            script.contains(HOME_LINE),
            "the recipe takes the home folder from the password database"
        );
        let script = script.replace(HOME_LINE, "home = sys.argv[2]");

        let owner = Owner::new();
        let home = owner.home.root.path().to_path_buf();
        let agents = owner.real("BuwizNsh/agents");
        fs::hard_link(
            owner.real("BuwizNsh/data/bir_data.db"),
            agents.join("alias.db"),
        )
        .expect("hard link");
        symlink(owner.real("BuwizNsh/data"), agents.join("to-data")).expect("link");
        #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
        let mut cases: Vec<(PathBuf, bool)> = vec![
            (owner.real("BuwizNsh/data/bir_data.db"), true),
            (owner.real("BuwizNsh/data/new/deeper.db"), true),
            (owner.real("BuwizNsh/migration"), true),
            (owner.real(&format!("{GROUP}/x.db")), true),
            (home.join("BuwizNsh/agents/../data/x.db"), true),
            (agents.join("alias.db"), true),
            (agents.join("to-data/x.db"), true),
            (agents.join("door.db"), false),
            (owner.real("BuwizNsh/data-copy/x.db"), false),
            (owner.real("BuwizNsh/agents"), false),
        ];
        // Any spelling of a name reaches the same folder on a case-insensitive file system (macOS by default); the guard
        // compares names case-sensitively on Linux (`inside`), so this case is only meaningful on macOS.
        #[cfg(target_os = "macos")]
        cases.push((home.join("buwiznsh/DATA/x.db"), true));
        #[cfg(target_os = "macos")]
        for (relative, refused) in [
            ("BuwizNsh/data/x.db", true),
            ("BuwizNsh/agents/x.db", false),
        ] {
            cases.push((
                PathBuf::from(format!(
                    "/System/Volumes/Data{}",
                    owner.real(relative).display()
                )),
                refused,
            ));
        }
        for (candidate, expected) in cases {
            let ran = Command::new("python3")
                .args(["-I", "-c", &script])
                .arg(&candidate)
                .arg(&home)
                .output();
            let output = match ran {
                Ok(output) => output,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return, // no python3 here
                Err(error) => panic!("python3: {error}"),
            };
            let recipe_refused = !output.status.success();
            assert_eq!(
                recipe_refused,
                expected,
                "the recipe on {}: {}",
                candidate.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                owner.home.refusal(&candidate).is_some(),
                expected,
                "the door guard on {}",
                candidate.display()
            );
        }
    }

    /// Windows cannot be verified here, so no database opens unless the caller named exactly that file. The rule is
    /// pure: this runs it as a system without the Unix lookups would.
    #[test]
    fn where_nothing_can_be_verified_only_a_named_database_opens() {
        let folder = tempfile::tempdir().expect("temp");
        let db = folder.path().join("bir_data.db");
        let named = || Some(db.clone().into_os_string());
        let why = unverified_refusal(false, &db, None).expect("nothing named");
        assert!(why.contains("BIR_DATABASE_PATH"), "{why}");
        assert!(unverified_refusal(false, &db, Some("".into())).is_some(), "an empty name names nothing");
        assert!(
            unverified_refusal(false, &folder.path().join("other.db"), named()).is_some(),
            "another file than the one named"
        );
        assert_eq!(unverified_refusal(false, &db, named()), None, "the file named");
        // Spelled with a `..` in between, it is still the same file.
        let detour = folder.path().join("x/../bir_data.db");
        assert_eq!(unverified_refusal(false, &detour, named()), None);
        // Where the lookups exist (Unix) the rule never applies.
        assert_eq!(unverified_refusal(true, &db, None), None);
    }

    /// The protected folders hang off the account's home folder from the password database, not `$HOME`:
    /// a process started with `HOME=/tmp/x` still protects the owner's real ones.
    #[cfg(unix)]
    #[test]
    fn the_protected_folders_ignore_home() {
        let elsewhere = tempfile::tempdir().expect("elsewhere");
        let account = account_home().expect("the account has a home folder");
        assert!(account.is_absolute(), "{}", account.display());
        temp_env::with_var("HOME", Some(elsewhere.path()), || {
            assert_eq!(account_home().as_deref(), Some(account.as_path()));
            let folders = protected_folders_here().expect("folders");
            assert_eq!(folders.len(), 3);
            for folder in folders {
                assert!(folder.starts_with(&account), "{}", folder.display());
                assert!(
                    !folder.starts_with(elsewhere.path()),
                    "{}",
                    folder.display()
                );
            }
        });
    }
}
