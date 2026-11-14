//! The two guards that stand between the TSX screens and the owner's data or the BIR.
//!
//! * The **send guard** ([`send_guard`]) runs once, before the first database open and before the
//!   submission worker starts. It forces a dry run, drops the live acknowledgement, and decides whether the
//!   worker may start at all. The one way out of a dry run is the mock BIR server on loopback.
//! * The **database guard** ([`database_refusal`]) refuses to open a database inside the owner's data
//!   folders. It is checked before every host call, so a lazy open can never get around it.
//!
//! Both are plain functions over the environment and a home folder, so their tests use temp folders and
//! never touch a real path. Contract: `docs/plans/nsh-tsx-migration.md`, "The door".

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
    // SAFETY: the guard runs once, at the boot of the door, before the database and the worker exist; the
    // environment is only read by the transport afterwards.
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
fn protected_folders(home: &Path) -> [PathBuf; 3] {
    [
        home.join("Library/Group Containers/group.dev.goldcoders.bir"),
        home.join("BuwizNsh/data"),
        home.join("BuwizNsh/migration"),
    ]
}

/// Why the door must not open `path` as the database, or `None` when it may.
///
/// `path` is made absolute and resolved through symlinks (as far as it exists, then the rest by name), so
/// `..` and links cannot reach a protected folder. With no `home` the owner's folders cannot be named, and
/// the answer is a refusal.
pub fn database_refusal(path: &Path, home: Option<&Path>) -> Option<String> {
    let Some(home) = home else {
        return Some(
            "Cannot tell where the owner's data folders are (no home folder), so no database is opened."
                .into(),
        );
    };
    let canonical = canonical_target(path);
    for folder in protected_folders(home) {
        let folder = canonical_target(&folder);
        if inside(&canonical, &folder) {
            return Some(format!(
                "Refusing to open {}: it is inside {}, the owner's data. Copy a database to ~/BuwizNsh/agents/<track>.db and point BIR_DATABASE_PATH there.",
                canonical.display(),
                folder.display()
            ));
        }
    }
    None
}

/// [`database_refusal`] for the real home folder.
pub fn database_refusal_here(path: &Path) -> Option<String> {
    database_refusal(path, std::env::home_dir().as_deref())
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
            database_refusal(path, Some(self.root.path()))
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
        let why = database_refusal(Path::new("/somewhere/else.db"), None).expect("refused");
        assert!(why.contains("no home folder"), "{why}");
    }
}
