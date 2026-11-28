//! The two guards that stand between the TSX screens and the owner's data or the BIR.
//!
//! * The **send guard** ([`send_guard`]) runs once, before the first database open and before the
//!   submission worker starts. A desktop release has nowhere to send a return ([`SendMode::Off`]): bir-core's
//!   `direct-bir` feature, the only code that files with the BIR itself, is off in this app. Development,
//!   test, agent and mobile runs stay dry unless a local mock is selected.
//! * The **database guard** ([`database_refusal`]) refuses to open a database inside the owner's data
//!   folders, by name and by device and inode. It is checked before every host call, so a lazy open can
//!   never get around it. The folders hang off the home folder of the account's real uid, not `$HOME`. In a
//!   test environment the NSH app's own data folder is one of them ([`protected_folders_for`]): a dev, test or
//!   agent run never opens the shipped app's real database.
//!
//! Both are plain functions over the environment and a list of folders, so their tests use temp folders and
//! never touch a real path. Contract: `docs/plans/nsh-tsx-migration.md`, "The door".
//!
//! **Platforms.** The database guard is complete on Unix (macOS, Linux) and Windows. On Unix the account's real
//! home folder comes from the password database, and a folder or file is known by device and inode. On Windows the
//! profile folder comes from the process's own token (`GetUserProfileDirectoryW`), never from the `USERPROFILE`
//! variable a caller can change, and a folder or file is known by its volume serial number and file index
//! (`GetFileInformationByHandle`), which junctions, symlinks, 8.3 short names, `\\?\` prefixes and hard links do
//! not change. Where neither exists, the guard opens a database only when the caller names it with
//! `BIR_DATABASE_PATH` ([`unverified_refusal`]).
//!
//! **Environment.** Android debug launch configuration is loaded before this guard (see `door::process`).

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use bir_core::dev_mode::SubmissionMode;

/// What the send guard decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendMode {
    /// The desktop release uses the configured filing transport. Only a build with bir-core's `direct-bir` feature,
    /// which no build that ships has.
    Live,
    /// This build has nowhere to send a return (`bir_core::transport::sending_unavailable`). Everything else works: the
    /// worker starts for its other jobs, leaves queued returns alone, and the app refuses to queue new ones.
    Off,
    /// `BIR_SFTP_DRY_RUN=1`: a queued return is "sent" to a folder on this computer, nothing leaves it.
    DryRun,
    /// The mock BIR server is listening on literal `127.0.0.1` or `::1`.
    Mock { host: String, port: u16 },
    /// The local mock relay (`just mock-relay`, docs/relay.md) is listening at `url`, a loopback `http://` origin,
    /// and the transport sends there, and nowhere else. The relay may hand the file on to the mock BIR server.
    Relay { url: String },
    /// Returns go to the BIR through the Buwiz server at `url`. A release build on a desktop: its configured server,
    /// production unless the build says otherwise. A development build asked with `BUWIZ_RELAY_SEND=1`: a server on
    /// this computer, trusted through a test key and listening; whether anything reaches the BIR is that server's
    /// mode.
    Buwiz { url: String },
    /// Sending is not a dry run and not the mock: the submission worker does not start. Why.
    Refused(String),
}

impl SendMode {
    /// Whether the submission worker may start.
    pub fn worker_may_start(&self) -> bool {
        !matches!(self, SendMode::Refused(_))
    }

    /// The word TSX sees: `live`, `off`, `dry_run`, `mock`, `relay`, `buwiz` or `refused`.
    pub fn label(&self) -> &'static str {
        match self {
            SendMode::Live => "live",
            SendMode::Off => "off",
            SendMode::DryRun => "dry_run",
            SendMode::Mock { .. } => "mock",
            SendMode::Relay { .. } => "relay",
            SendMode::Buwiz { .. } => "buwiz",
            SendMode::Refused(_) => "refused",
        }
    }

    /// A sentence for the status, when there is something to say.
    pub fn note(&self) -> Option<String> {
        match self {
            SendMode::Live | SendMode::DryRun => None,
            SendMode::Off => Some(bir_core::transport::SENDING_UNAVAILABLE.into()),
            SendMode::Mock { host, port } => Some(format!(
                "Sending to the local mock endpoint at {host}:{port}. Nothing reaches the BIR."
            )),
            SendMode::Relay { url } => Some(format!(
                "Sending through the mock relay at {url}. Nothing reaches the BIR."
            )),
            SendMode::Buwiz { url } if cfg!(debug_assertions) => Some(format!(
                "Sending through the Buwiz server at {url} on this computer. Whether anything reaches the BIR is that server's mode."
            )),
            SendMode::Buwiz { .. } => None,
            SendMode::Refused(reason) => Some(reason.clone()),
        }
    }
}

/// How long the mock's port gets to answer.
const LISTEN_TIMEOUT: Duration = Duration::from_millis(400);

/// On guarded launches, set `BIR_SFTP_DRY_RUN=1`, remove `BIR_SFTP_LIVE`, then check that the transport really is a dry run and
/// that the Dev Mode panel does not point at the test server. Mock mode is the only exception: a listening
/// loopback `BIR_SFTP_HOST` leaves the environment as the `mock-bir-app` recipe made it. An explicit
/// `BIR_RELAY_URL` must instead pass the loopback/health guard and never falls back to SFTP.
///
/// Android enforces these switches through in-process configuration; desktop registration
/// changes the environment. Call it once, before the database opens (the process boot does).
pub fn send_guard() -> SendMode {
    send_guard_for(
        restrict_sending(cfg!(debug_assertions)),
        port_is_listening,
        mock_health,
        bir_core::transport::is_sftp_dry_run,
        bir_core::transport::sending_unavailable,
    )
}

fn restrict_sending(debug_build: bool) -> bool {
    restricted_launch(
        System::this(),
        debug_build,
        bir_core::db::test_environment(),
        matches!(bir_core::config::var("GPUI_AGENT").as_deref(), Ok("1" | "true")),
    )
}

/// The system an app build runs on, for [`restricted_launch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum System {
    MacOs,
    Windows,
    Linux,
    Ios,
    Android,
}

impl System {
    #[cfg(test)]
    const ALL: [System; 5] = [System::MacOs, System::Windows, System::Linux, System::Ios, System::Android];

    fn this() -> System {
        if cfg!(target_os = "ios") {
            System::Ios
        } else if cfg!(target_os = "android") {
            System::Android
        } else if cfg!(target_os = "windows") {
            System::Windows
        } else if cfg!(target_os = "linux") {
            System::Linux
        } else {
            System::MacOs
        }
    }
}

/// Whether this launch may only simulate, or send through a server on this computer: a debug build
/// (`cfg!(debug_assertions)`), a test environment (`EBIR_TEST_ENV`) or an agent-driven launch (`GPUI_AGENT`). The
/// system does not change it: a release build (an App Store or TestFlight build, a Play bundle, a desktop package:
/// all built without `debug_assertions`) files through the Buwiz relay on iPhone, iPad and Android exactly as on
/// macOS, Windows and Linux, and a development build never sends or registers through production on any of them.
fn restricted_launch(_system: System, debug_build: bool, test_environment: bool, agent: bool) -> bool {
    debug_build || test_environment || agent
}

/// [`send_guard`] with the "is anything listening there" probe given, for tests.
#[cfg(test)]
pub(crate) fn send_guard_with(listening: impl Fn(&str, u16) -> bool) -> SendMode {
    send_guard_with_health(listening, mock_health)
}

#[cfg(test)]
fn send_guard_with_health(
    listening: impl Fn(&str, u16) -> bool,
    healthy: impl Fn(&str, u16) -> Result<bool, String>,
) -> SendMode {
    send_guard_for(
        true,
        listening,
        healthy,
        bir_core::transport::is_sftp_dry_run,
        bir_core::transport::sending_unavailable,
    )
}

fn send_guard_for(
    restricted: bool,
    listening: impl Fn(&str, u16) -> bool,
    healthy: impl Fn(&str, u16) -> Result<bool, String>,
    transport_dry_run: impl FnOnce() -> bool,
    nowhere_to_send: impl FnOnce() -> bool,
) -> SendMode {
    if restricted {
        #[cfg(target_os = "android")]
        bir_core::config::force("BIR_SFTP_LIVE", None);
        #[cfg(not(target_os = "android"))]
        // SAFETY: desktop registration runs before NSH threads, database and worker startup.
        unsafe {
            std::env::remove_var("BIR_SFTP_LIVE")
        };
    }
    let relay = relay_target(&listening, &healthy);
    if let Some(Ok(url)) = &relay {
        #[cfg(target_os = "android")]
        bir_core::config::force(bir_core::transport::RELAY_URL_ENV, Some(url));
        #[cfg(not(target_os = "android"))]
        // SAFETY: registration precedes worker startup, as above.
        unsafe {
            std::env::set_var(bir_core::transport::RELAY_URL_ENV, url)
        };
    }
    if !restricted {
        return relay_verdict(relay).unwrap_or_else(|| {
            if transport_dry_run() {
                SendMode::DryRun
            } else if nowhere_to_send() {
                SendMode::Off
            } else if let Some(buwiz) = release_relay() {
                buwiz
            } else {
                SendMode::Live
            }
        });
    }
    // The local Buwiz relay is asked for like the mock relay; the mock relay, when set, comes first (the transport
    // decides in that order too).
    let buwiz = if relay.is_none() { buwiz_target(&listening) } else { None };
    let mock = mock_target(&listening);
    if !matches!(relay, Some(Ok(_))) && !matches!(buwiz, Some(Ok(_))) && mock.is_none() {
        #[cfg(target_os = "android")]
        bir_core::config::force("BIR_SFTP_DRY_RUN", Some("1"));
        #[cfg(not(target_os = "android"))]
        // SAFETY: as above.
        unsafe {
            std::env::set_var("BIR_SFTP_DRY_RUN", "1")
        };
    }
    let panel = bir_core::dev_mode::panel_submission_mode();
    relay_verdict(relay)
        .or_else(|| buwiz_verdict(buwiz))
        .unwrap_or_else(|| verdict(mock, transport_dry_run(), panel))
}

/// A release build on a desktop files through the Buwiz relay (`bir_core::transport::sends_through_buwiz`): the
/// mode names the configured Buwiz server, or refuses to start the worker when that server may not be used.
/// `None` when the send does not go through Buwiz (a build with `direct-bir`).
fn release_relay() -> Option<SendMode> {
    if !bir_core::transport::sends_through_buwiz() {
        return None;
    }
    Some(match bir_core::relay::server() {
        Ok(url) => SendMode::Buwiz { url },
        Err(error) => SendMode::Refused(format!("{error} The submission worker was not started.")),
    })
}

/// The decision when a development build is asked to send through the Buwiz relay (`buwiz`: the server's origin when
/// it is allowed and listening, else why not): it is that server or nothing. `None` when it was not asked for.
pub(crate) fn buwiz_verdict(buwiz: Option<Result<String, String>>) -> Option<SendMode> {
    Some(match buwiz? {
        Ok(url) => SendMode::Buwiz { url },
        Err(why) => SendMode::Refused(why),
    })
}

/// The Buwiz server a development build may send through (`bir_core::relay::server`: on this computer, trusted
/// through a test key) when its port answers; the refusal otherwise. `None` unless `BUWIZ_RELAY_SEND=1` in a
/// development build.
fn buwiz_target(listening: &impl Fn(&str, u16) -> bool) -> Option<Result<String, String>> {
    if !bir_core::transport::buwiz_relay_requested() {
        return None;
    }
    let url = match bir_core::relay::server() {
        Ok(url) => url,
        Err(error) => return Some(Err(format!("{error} The submission worker was not started."))),
    };
    let authority = url.split("://").nth(1).unwrap_or_default().split('/').next().unwrap_or_default();
    let (host, port) = match authority.strip_prefix('[') {
        Some(ipv6) => {
            let (host, rest) = ipv6.split_once(']').unwrap_or((ipv6, ""));
            (host, rest.strip_prefix(':').and_then(|port| port.parse::<u16>().ok()))
        }
        None => match authority.rsplit_once(':') {
            Some((host, port)) => (host, port.parse::<u16>().ok()),
            None => (authority, None),
        },
    };
    let port = port.unwrap_or(if url.starts_with("https://") { 443 } else { 80 });
    let hosts: &[&str] = if host.eq_ignore_ascii_case("localhost") { &["127.0.0.1", "::1"] } else { &[host] };
    if hosts.iter().any(|host| listening(host, port)) {
        Some(Ok(url))
    } else {
        Some(Err(format!(
            "The Buwiz server at {url} is not listening; the submission worker was not started."
        )))
    }
}

/// The decision when `BIR_RELAY_URL` is set (`relay`: its origin when it is a listening loopback one, else why
/// not): the transport sends there before anything else, so it is the relay or nothing. `None` when it is unset.
/// Pure, so every branch is tested.
pub(crate) fn relay_verdict(relay: Option<Result<String, String>>) -> Option<SendMode> {
    // The relay environment overrides the panel, including a saved dry run.
    let relay = relay?;
    Some(match relay {
        Ok(url) => SendMode::Relay { url },
        Err(why) => SendMode::Refused(why),
    })
}

/// `BIR_RELAY_URL` when it is a loopback `http://` origin whose port answers; the refusal when it is set otherwise.
fn relay_target(
    listening: &impl Fn(&str, u16) -> bool,
    healthy: &impl Fn(&str, u16) -> Result<bool, String>,
) -> Option<Result<String, String>> {
    let url = match bir_core::transport::relay_url().or_else(|| {
        match bir_core::config::var(bir_core::transport::RELAY_URL_ENV) {
            Err(std::env::VarError::NotPresent) => None,
            _ => Some(Err(bir_core::transport::TransportError::Config(
                "Empty or invalid BIR_RELAY_URL".into(),
            ))),
        }
    })? {
        Ok(url) => url,
        Err(error) => return Some(Err(error.to_string())),
    };
    let authority = url.trim_start_matches("http://");
    let (host, port) = if let Some(ipv6) = authority.strip_prefix('[') {
        let (host, suffix) = ipv6.split_once(']')?;
        (host, if suffix.is_empty() { Some(80) } else { suffix.strip_prefix(':')?.parse::<u16>().ok() })
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, port.parse::<u16>().ok()),
            None => (authority, Some(80)),
        }
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let localhost = host.eq_ignore_ascii_case("localhost");
    let hosts: &[&str] = if localhost { &["127.0.0.1", "::1"] } else { &[host] };
    let mut health_errors = Vec::new();
    if let Some(port) = port {
        for host in hosts.iter().filter(|host| listening(host, port)) {
            match healthy(host, port) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(why) => {
                    if !health_errors.contains(&why) {
                        health_errors.push(why);
                    }
                    continue;
                }
            }
            if localhost {
                let host = if host.contains(':') { format!("[{host}]") } else { host.to_string() };
                return Some(Ok(format!("http://{host}:{port}")));
            }
            return Some(Ok(url));
        }
    }
    if !health_errors.is_empty() {
        return Some(Err(format!("{} The submission worker was not started.", health_errors.join(" "))));
    }
    Some(Err(format!(
            "The mock relay at {url} is not listening or /v1/health did not identify a mock; the submission worker was not started."
        )))
}

fn mock_health(host: &str, port: u16) -> Result<bool, String> {
    let probe = || -> Option<Result<bool, String>> {
        let ip = host.parse::<IpAddr>().ok()?;
        let address = SocketAddr::new(ip, port);
        let mut stream = TcpStream::connect_timeout(&address, LISTEN_TIMEOUT).ok()?;
        stream.set_read_timeout(Some(LISTEN_TIMEOUT)).ok()?;
        stream.set_write_timeout(Some(LISTEN_TIMEOUT)).ok()?;
        let request = format!("GET /v1/health HTTP/1.0\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).ok()?;
        let mut response = Vec::new();
        stream.take(8193).read_to_end(&mut response).ok()?;
        if response.len() > 8192 {
            return None;
        }
        let end = response.windows(4).position(|part| part == b"\r\n\r\n")?;
        let head = std::str::from_utf8(&response[..end]).ok()?;
        if head.lines().next()?.split_whitespace().nth(1) != Some("200") {
            return None;
        }
        let body: serde_json::Value = serde_json::from_slice(&response[end + 4..]).ok()?;
        Some(bir_core::transport::check_mock_relay_health(&body, address).map(|()| true))
    };
    probe().unwrap_or(Ok(false))
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
    let host = bir_core::config::var("BIR_SFTP_HOST").ok()?;
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
/// the account has no entry. On Windows the profile folder of the account this process's token belongs to, from
/// Windows itself (`GetUserProfileDirectoryW`), never from `USERPROFILE`: a process started with
/// `USERPROFILE=C:\x` still protects the owner's real folders.
pub fn account_home() -> Option<PathBuf> {
    // iOS: an app has no account entry of its own (the simulator's password database may not know the uid either);
    // its home is the sandbox container the system sets as HOME at launch, where every folder it can name lives.
    #[cfg(target_os = "ios")]
    {
        std::env::var_os("HOME").map(PathBuf::from).filter(|home| home.is_absolute())
    }
    #[cfg(target_os = "android")]
    {
        Some(bir_core::platform::data_dir())
    }
    #[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
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
    #[cfg(windows)]
    {
        windows::profile_folder()
    }
    #[cfg(not(any(unix, windows)))]
    {
        std::env::home_dir()
    }
}

/// Where the NSH app keeps its own data (bir-core's `platform::data_dir` of the plain build): the default under
/// `home` (macOS and iOS `Library/Application Support`, Windows `AppData\Local`, elsewhere `.local/share`), and where the
/// variable the app reads (`LOCALAPPDATA` on Windows, an absolute `XDG_DATA_HOME` elsewhere off macOS) moves it.
pub fn app_data_folders(home: &Path) -> Vec<PathBuf> {
    let id = bir_core::platform::NSH_APP_ID;
    let (default, moved) = if cfg!(any(target_os = "macos", target_os = "ios")) {
        (home.join("Library/Application Support"), None)
    } else if cfg!(windows) {
        (home.join("AppData/Local"), std::env::var_os("LOCALAPPDATA"))
    } else {
        (home.join(".local/share"), std::env::var_os("XDG_DATA_HOME"))
    };
    let mut folders = vec![default.join(id)];
    if let Some(moved) = moved.map(PathBuf::from).filter(|path| path.is_absolute()) {
        folders.push(moved.join(id));
    }
    folders
}

/// The folders the guard protects under `home`: the owner's ([`protected_folders`]), and in a test environment
/// (`test_environment`) the NSH app's own data folders too ([`app_data_folders`]), so a dev, test or agent run never
/// opens the shipped app's real database, even named. Outside a test environment that is the database the app opens.
pub fn protected_folders_for(home: &Path, test_environment: bool) -> Vec<PathBuf> {
    let mut folders = protected_folders(home).to_vec();
    if test_environment {
        folders.extend(app_data_folders(home));
    }
    folders
}

/// The protected folders, under the account's real home folder. `None` when that cannot be found.
fn protected_folders_here() -> Option<Vec<PathBuf>> {
    #[cfg(test)]
    if let Some(folders) = TEST_PROTECTED.with(|slot| slot.borrow().clone()) {
        return Some(folders);
    }
    account_home().map(|home| protected_folders_for(&home, crate::door::test_environment()))
}

#[cfg(test)]
thread_local! {
    /// Test seam: the protected folders of this thread, in place of the real account's (a test never
    /// reads the owner's real folders).
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; Clippy misreads Android's thread_local expansion"))]
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
/// * **By identity** (Unix: device and inode; Windows: volume serial number and file index; see [`identity`]).
///   A protected folder that exists is known by its identity, which no
///   spelling changes: macOS reaches the same folder as `/Users/<name>/x` and `/System/Volumes/Data/Users/<name>/x`
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

/// The folder under an Android engine's data folder where a test run may keep a named database that outlives its
/// process (`BIR_DATABASE_PATH`, an opted-in development build's dev list).
pub const ANDROID_TEST_DATA: &str = "test-data";

/// On Android in a test run, a database named with `BIR_DATABASE_PATH` must sit inside `<data_dir>/test-data`: never the
/// app's own database (`<data_dir>/bir_data.db`), nor anywhere else. Pure, so it runs on any system.
#[cfg_attr(not(target_os = "android"), allow(dead_code, reason = "called on Android; tested everywhere"))]
pub(crate) fn android_test_database_refusal(path: &Path, data_dir: &Path) -> Option<String> {
    let allowed = canonical_target(&data_dir.join(ANDROID_TEST_DATA));
    let canonical = canonical_target(path);
    if canonical != allowed && inside(&canonical, &allowed) {
        return None;
    }
    Some(format!(
        "Refusing to open {}: a test run names a database only inside {}.",
        canonical.display(),
        allowed.display()
    ))
}

/// [`database_refusal`] for the owner's real folders. With no home folder to name them, the answer is a
/// refusal.
pub fn database_refusal_here(path: &Path) -> Option<String> {
    #[cfg(target_os = "android")]
    if crate::door::test_environment()
        && bir_core::config::var(DATABASE_PATH_ENV).is_ok_and(|value| !value.is_empty())
        && let Some(why) = android_test_database_refusal(path, &bir_core::platform::data_dir())
    {
        return Some(why);
    }
    // A test that names its own "home" (the seam) checks the by-name rules on any system.
    #[cfg(test)]
    let seamed = TEST_PROTECTED.with(|slot| slot.borrow().is_some());
    #[cfg(not(test))]
    let seamed = false;
    if !seamed
        && let Some(why) = unverified_refusal(
            cfg!(any(unix, windows)),
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

/// Where the guard cannot do what it promises (`can_verify` is false: a system that is neither Unix nor Windows, so
/// has no account lookup and no file identity here), it opens no database the caller has not named: `path` must be the file `BIR_DATABASE_PATH`
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

#[cfg(windows)]
fn identity(path: &Path) -> Option<Identity> {
    windows::identity(path)
}

/// Elsewhere there is no file identity: the guard is by name only (and [`unverified_refusal`] applies).
#[cfg(not(any(unix, windows)))]
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

#[cfg(windows)]
fn hard_linked_file_identity(path: &Path) -> Option<Identity> {
    windows::hard_linked_file_identity(path)
}

#[cfg(not(any(unix, windows)))]
fn hard_linked_file_identity(_path: &Path) -> Option<Identity> {
    None
}

/// The Windows side of the database guard, from Windows itself: the profile folder of the process's account, and a
/// file's identity (volume serial number, file index) and link count.
#[cfg(windows)]
mod windows {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::TOKEN_QUERY;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows_sys::Win32::UI::Shell::GetUserProfileDirectoryW;

    use super::Identity;

    /// A handle this module opened, closed when it goes.
    struct Owned(HANDLE);

    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: the handle was opened by this module, is valid, and is closed only here, once.
            unsafe { CloseHandle(self.0) };
        }
    }

    /// The profile folder of the account the process's token belongs to (`C:\Users\<name>`), as Windows records
    /// it for that account; the `USERPROFILE` variable plays no part. `None` when Windows has none to give.
    pub(super) fn profile_folder() -> Option<PathBuf> {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: the current-process pseudo-handle needs no closing; on success `token` holds a handle that `Owned`
        // closes.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return None;
        }
        let token = Owned(token);
        let mut size: u32 = 0;
        // SAFETY: with no buffer the call only writes the size it needs, in UTF-16 units, to `size`.
        unsafe { GetUserProfileDirectoryW(token.0, std::ptr::null_mut(), &mut size) };
        if size == 0 {
            return None;
        }
        let mut buffer = vec![0u16; size as usize];
        // SAFETY: `buffer` holds the `size` units the call asked for and outlives it.
        if unsafe { GetUserProfileDirectoryW(token.0, buffer.as_mut_ptr(), &mut size) } == 0 {
            return None;
        }
        let len = buffer.iter().position(|&unit| unit == 0).unwrap_or(buffer.len());
        (len > 0).then(|| PathBuf::from(OsString::from_wide(&buffer[..len])))
    }

    /// What Windows knows of the file or folder `path` reaches (a symlink or junction on the way is followed), or
    /// `None` when it cannot be opened.
    fn information(path: &Path) -> Option<BY_HANDLE_FILE_INFORMATION> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call. It asks for attributes only and
        // shares everything, so an open database is not disturbed; backup semantics lets it open a folder too.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let handle = Owned(handle);
        // SAFETY: all-zero is a valid BY_HANDLE_FILE_INFORMATION, which the call below fills.
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: `handle` is open and `info` is ours to write.
        (unsafe { GetFileInformationByHandle(handle.0, &mut info) } != 0).then_some(info)
    }

    fn key(info: &BY_HANDLE_FILE_INFORMATION) -> Identity {
        (
            u64::from(info.dwVolumeSerialNumber),
            (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        )
    }

    /// The file or folder `path` reaches, by volume and file index.
    pub(super) fn identity(path: &Path) -> Option<Identity> {
        information(path).map(|info| key(&info))
    }

    /// The identity of `path` when it is an existing regular file with more than one link.
    pub(super) fn hard_linked_file_identity(path: &Path) -> Option<Identity> {
        if !std::fs::metadata(path).ok()?.is_file() {
            return None;
        }
        let info = information(path)?;
        (info.nNumberOfLinks > 1).then(|| key(&info))
    }
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

    #[test]
    fn an_android_test_run_opens_a_named_database_only_inside_test_data() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("files/nsh/dev.goldcoders.bir");
        assert_eq!(android_test_database_refusal(&data.join("test-data/bir_data.db"), &data), None);
        for refused in [
            data.join("bir_data.db"),
            data.join("test-data"),
            data.join("test-data/../bir_data.db"),
            data.join("other/bir_data.db"),
            dir.path().join("elsewhere.db"),
        ] {
            assert!(android_test_database_refusal(&refused, &data).is_some(), "{}", refused.display());
        }
    }

    /// Every variable the send guard reads or writes, so a test runs with exactly the ones it names.
    fn sending_env<R>(set: &[(&str, &str)], body: impl FnOnce() -> R) -> R {
        let names = [
            "BIR_SFTP_DRY_RUN",
            "BIR_SFTP_LIVE",
            "BIR_SFTP_HOST",
            "BIR_SFTP_PORT",
            "BIR_RELAY_URL",
            "BUWIZ_RELAY_SEND",
            "BUWIZ_SERVER_URL",
            "BUWIZ_TRUST_KEYS",
            "EBIR_TEST_ENV",
            "GPUI_AGENT",
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

    /// Every system alike, phones included: only a release launch outside a test or agent environment may file; any
    /// debug, test or agent launch is restricted.
    #[test]
    fn only_a_plain_release_launch_may_file_on_any_system() {
        for system in System::ALL {
            assert!(!restricted_launch(system, false, false, false), "{system:?}: a release build files");
            for debug_build in [false, true] {
                for test_environment in [false, true] {
                    for agent in [false, true] {
                        assert_eq!(
                            restricted_launch(system, debug_build, test_environment, agent),
                            debug_build || test_environment || agent,
                            "{system:?}: debug {debug_build}, test {test_environment}, agent {agent}"
                        );
                    }
                }
            }
        }
    }

    /// A release launch, on whatever system the test runs, leaves the filing transport in control (it ran on the
    /// desktops only while phones were held to a dry run).
    #[test]
    fn a_release_launch_leaves_the_filing_transport_in_control() {
        // A dry run wins; then a build with nowhere to send is off; only a build that can file itself is live.
        for (transport_is_dry, nowhere_to_send, expected) in [
            (false, false, SendMode::Live),
            (false, true, SendMode::Off),
            (true, true, SendMode::DryRun),
            (true, false, SendMode::DryRun),
        ] {
            sending_env(&[], || {
                let mode = send_guard_for(
                    restrict_sending(false),
                    |_, _| panic!("no mock requested"),
                    |_, _| panic!("no relay requested"),
                    || transport_is_dry,
                    || nowhere_to_send,
                );
                assert_eq!(mode, expected);
                assert!(mode.worker_may_start());
                assert_eq!(dry_run(), None);
                assert_eq!(live(), None);
            });
        }
    }

    /// The local Buwiz relay is that server or nothing, like the mock relay.
    #[test]
    fn the_local_buwiz_relay_is_that_server_or_a_refusal() {
        assert_eq!(buwiz_verdict(None), None);
        let mode = buwiz_verdict(Some(Ok("http://localhost:3008".into()))).unwrap();
        assert_eq!(mode, SendMode::Buwiz { url: "http://localhost:3008".into() });
        assert!(mode.worker_may_start());
        assert_eq!(mode.label(), "buwiz");
        assert!(mode.note().unwrap().contains("http://localhost:3008"));
        let refused = buwiz_verdict(Some(Err("why".into()))).unwrap();
        assert_eq!(refused, SendMode::Refused("why".into()));
        assert!(!refused.worker_may_start());
    }

    /// Asked for with nothing listening, a development build refuses to start the worker; a server that is not on
    /// this computer, or production's trust key, is refused before any port is tried.
    #[test]
    fn the_local_buwiz_relay_needs_a_listening_server_on_this_computer_and_a_test_key() {
        let test_key = "dGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXk";
        sending_env(
            &[("BUWIZ_RELAY_SEND", "1"), ("BUWIZ_SERVER_URL", "http://localhost:3008"), ("BUWIZ_TRUST_KEYS", test_key)],
            || {
                assert_eq!(buwiz_target(&|_, port| port == 3008), Some(Ok("http://localhost:3008".into())));
                let down = buwiz_target(&|_, _| false).unwrap().unwrap_err();
                assert!(down.contains("not listening"), "{down}");
            },
        );
        sending_env(&[("BUWIZ_RELAY_SEND", "1"), ("BUWIZ_SERVER_URL", "http://localhost:3008")], || {
            let refused = buwiz_target(&|_, _| panic!("no port is tried")).unwrap().unwrap_err();
            assert!(refused.contains("test key"), "{refused}");
        });
        sending_env(
            &[("BUWIZ_RELAY_SEND", "1"), ("BUWIZ_SERVER_URL", "https://api.buwiz.com"), ("BUWIZ_TRUST_KEYS", test_key)],
            || {
                let refused = buwiz_target(&|_, _| panic!("no port is tried")).unwrap().unwrap_err();
                assert!(refused.contains("on this computer"), "{refused}");
            },
        );
        sending_env(&[], || assert_eq!(buwiz_target(&|_, _| panic!("not asked for")), None));
    }

    /// Off is not a refusal: the worker starts (its other jobs run), the screens read `off`, and the note says why.
    #[test]
    fn off_starts_the_worker_and_says_sending_is_not_available() {
        let mode = SendMode::Off;
        assert!(mode.worker_may_start());
        assert_eq!(mode.label(), "off");
        assert_eq!(mode.note().as_deref(), Some(bir_core::transport::SENDING_UNAVAILABLE));
    }

    /// The app this crate builds cannot file with the BIR itself: bir-core's `direct-bir` feature is off here.
    #[test]
    fn this_app_is_built_without_direct_filing() {
        assert!(!bir_core::transport::direct_filing_built_in());
    }

    #[test]
    fn test_and_agent_launches_cannot_go_live_in_release() {
        for flag in [("EBIR_TEST_ENV", "1"), ("GPUI_AGENT", "1")] {
            sending_env(&[flag], || {
                assert!(restrict_sending(false));
                let mode = send_guard_for(
                    restrict_sending(false),
                    |_, _| panic!("no mock requested"),
                    |_, _| panic!("no relay requested"),
                    bir_core::transport::is_sftp_dry_run,
                    || panic!("a dry run is decided first"),
                );
                assert_eq!(mode, SendMode::DryRun);
                assert_eq!(dry_run().as_deref(), Some("1"));
            });
        }
    }

    #[test]
    fn non_loopback_or_invalid_relay_refuses_even_when_dry_run_is_set() {
        for url in ["http://10.0.2.2:8787", "https://127.0.0.1:8787", "", "http://127.0.0.1:8787@evil.test"] {
            sending_env(&[("BIR_RELAY_URL", url), ("BIR_SFTP_DRY_RUN", "1"), ("BIR_SFTP_LIVE", "1")], || {
                assert!(matches!(send_guard_with(|_, _| panic!("invalid relay must not connect")), SendMode::Refused(_)));
                assert_eq!(live(), None);
            });
        }
    }

    #[test]
    fn relay_requires_both_a_listener_and_mock_health() {
        sending_env(&[("BIR_RELAY_URL", "http://127.0.0.1:8787")], || {
            assert!(matches!(send_guard_with_health(|_, _| false, |_, _| panic!("closed port")), SendMode::Refused(_)));
            assert!(matches!(send_guard_with_health(|_, _| true, |_, _| Ok(false)), SendMode::Refused(_)));
            let mode = send_guard_with_health(|host, port| host == "127.0.0.1" && port == 8787, |_, _| Ok(true));
            assert_eq!(mode.label(), "relay");
            assert!(mode.worker_may_start());
        });
    }

    #[test]
    fn relay_guard_accepts_ipv6_and_localhost_origins() {
        for (url, host, port) in [
            ("http://[::1]", "::1", 80),
            ("http://[::1]:8787", "::1", 8787),
            ("http://localhost:8787", "127.0.0.1", 8787),
        ] {
            sending_env(&[("BIR_RELAY_URL", url)], || {
                assert!(matches!(
                    send_guard_with_health(|h, p| h == host && p == port, |_, _| Ok(true)),
                    SendMode::Relay { .. }
                ));
            });
        }
    }

    #[test]
    fn health_probe_requires_successful_mock_json() {
        for (status, body, expected) in [
            ("200 OK", r#"{"service":"buwiz-mock-relay","version":1,"ok":true,"mock":true}"#, true),
            ("200 OK", r#"{"ok":true,"mock":true}"#, false),
            ("200 OK", r#"{"mock":false}"#, false),
            ("200 OK", "invalid", false),
            ("200 OK", "", false),
            ("503 Unavailable", r#"{"mock":true}"#, false),
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut request = Vec::new();
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let mut chunk = [0; 1024];
                    let read = stream.read(&mut chunk).unwrap();
                    assert!(read > 0, "connection closed before the request headers ended");
                    request.extend_from_slice(&chunk[..read]);
                }
                write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            });
            let url = format!("http://127.0.0.1:{port}");
            sending_env(&[("BIR_RELAY_URL", &url)], || {
                let mode = send_guard_with_health(|_, _| true, mock_health);
                assert_eq!(mode.worker_may_start(), expected);
                if body == r#"{"ok":true,"mock":true}"# {
                    let SendMode::Refused(why) = mode else { panic!("legacy relay must refuse") };
                    assert!(why.contains("out of date"), "{why}");
                    assert!(why.contains(&format!("`just mock-relay --bind 127.0.0.1 --port {port}`, then restart the app")), "{why}");
                }
            });
            server.join().unwrap();
        }
    }

    fn localhost_listener(host: &str) {
        let listener = match std::net::TcpListener::bind((host, 0)) {
            Ok(listener) => listener,
            Err(error) if host == "::1" => {
                eprintln!("IPv6 loopback is unavailable: {error}");
                return;
            }
            Err(error) => panic!("IPv4 loopback is unavailable: {error}"),
        };
        let url = format!("http://localhost:{}", listener.local_addr().unwrap().port());
        let pinned = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            // The port check connects without sending; the health check sends a request.
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut request = [0; 1024];
                if stream.read(&mut request).unwrap() > 0 {
                    let body = r#"{"service":"buwiz-mock-relay","version":1,"ok":true,"mock":true}"#;
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                }
            }
        });
        sending_env(&[("BIR_RELAY_URL", &url)], || {
            assert_eq!(send_guard_with(port_is_listening), SendMode::Relay { url: pinned.clone() });
            assert_eq!(bir_core::transport::relay_url().unwrap().unwrap(), pinned);
        });
        server.join().unwrap();
    }

    #[test]
    fn localhost_accepts_an_ipv4_only_relay() {
        localhost_listener("127.0.0.1");
    }

    #[test]
    fn localhost_accepts_an_ipv6_only_relay() {
        localhost_listener("::1");
    }

    #[test]
    fn localhost_checks_both_loopback_families_when_the_first_is_not_a_mock() {
        sending_env(&[("BIR_RELAY_URL", "http://localhost:8787")], || {
            let healthy = |host: &str, port: u16| {
                assert!(matches!(host, "127.0.0.1" | "::1"));
                if host == "::1" { Ok(true) } else {
                    bir_core::transport::check_mock_relay_health(&serde_json::json!({"ok":true,"mock":true}), SocketAddr::new(host.parse().unwrap(), port)).map(|()| true)
                }
            };
            assert_eq!(relay_target(&|_, _| true, &healthy), Some(Ok("http://[::1]:8787".into())));
        });
    }

    #[test]
    fn a_legacy_relay_refusal_is_not_hidden_by_another_loopback_listener() {
        for (url, legacy_host) in [
            ("http://localhost:9000", "127.0.0.1"),
            ("http://localhost:9000", "::1"),
            ("http://[::1]:9000", "::1"),
        ] {
            sending_env(&[("BIR_RELAY_URL", url)], || {
                let healthy = |host: &str, port: u16| {
                    let body = if host == legacy_host {
                        serde_json::json!({"ok":true,"mock":true})
                    } else {
                        serde_json::json!({})
                    };
                    bir_core::transport::check_mock_relay_health(&body, SocketAddr::new(host.parse().unwrap(), port)).map(|()| true)
                };
                let mode = send_guard_with_health(|_, _| true, healthy);
                let SendMode::Refused(why) = mode else { panic!("legacy relay must refuse") };
                assert!(why.contains("out of date"), "{why}");
                assert!(why.contains(&format!("`just mock-relay --bind {legacy_host} --port 9000`, then restart the app")), "{why}");
            });
        }
    }

    #[test]
    fn a_dry_run_is_forced_and_the_live_acknowledgement_removed() {
        sending_env(&[("BIR_SFTP_LIVE", "1")], || {
            assert_eq!(send_guard_with(port_is_listening), SendMode::DryRun);
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
                let mode = send_guard_with(port_is_listening);
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
                assert_eq!(send_guard_with(port_is_listening), SendMode::DryRun);
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
    fn a_relay_url_decides_alone_and_only_a_listening_loopback_one_sends() {
        assert_eq!(relay_verdict(None), None);
        assert_eq!(
            relay_verdict(Some(Ok("http://127.0.0.1:8787".into()))),
            Some(SendMode::Relay { url: "http://127.0.0.1:8787".into() })
        );
        assert!(matches!(relay_verdict(Some(Err("no".into()))), Some(SendMode::Refused(_))));
    }

    #[test]
    fn a_listening_loopback_relay_is_the_relay_and_anything_else_refuses() {
        sending_env(&[("BIR_RELAY_URL", "http://127.0.0.1:8787")], || {
            assert_eq!(
                send_guard_with_health(|host, port| host == "127.0.0.1" && port == 8787, |_, _| Ok(true)),
                SendMode::Relay { url: "http://127.0.0.1:8787".into() }
            );
            assert_eq!(dry_run(), None);
        });
        sending_env(&[("BIR_RELAY_URL", "http://127.0.0.1:8787")], || {
            assert!(matches!(send_guard_with(|_, _| false), SendMode::Refused(_)));
            assert_eq!(dry_run().as_deref(), Some("1"));
        });
        sending_env(&[("BIR_RELAY_URL", "https://relay.buwiz.com")], || {
            assert!(matches!(send_guard_with(|_, _| true), SendMode::Refused(_)));
        });
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
            home.refusal(&home.path("Library/Group Containers/group.dev.goldcoders.bir.example/x.db")),
            None
        );
        assert_eq!(
            home.refusal(
                &home.path("Library/Application Support/dev.goldcoders.bir/bir_data.db")
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
    /// database in each, plus a folder that is not protected.
    #[cfg(any(unix, windows))]
    struct Owner {
        home: Home,
    }

    #[cfg(any(unix, windows))]
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

        /// The temp home as the file system names it (macOS puts /var behind /private; Windows gives the verbatim
        /// `\\?\C:\...` spelling).
        fn real(&self, relative: &str) -> PathBuf {
            // Joined name by name, so the separators are the system's: a verbatim Windows path takes `/` literally.
            let relative: PathBuf = relative.split('/').collect();
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

    /// The spellings Windows has for one folder or file are one identity to the guard: the verbatim `\\?\` path,
    /// a junction (which any user can make), an 8.3 short name, another case, and a hard link. Each reaches the
    /// owner's data and is refused; a junction or hard link to something else is not.
    #[cfg(windows)]
    #[test]
    fn windows_spellings_of_a_protected_folder_are_refused() {
        use std::process::Command;

        let owner = Owner::new();
        let data = owner.real("BuwizNsh/data");
        let agents = owner.real("BuwizNsh/agents");
        assert!(data.to_string_lossy().starts_with(r"\\?\"), "{}", data.display());
        for candidate in [data.join("bir_data.db"), data.join(r"new\deeper.db"), data.clone()] {
            assert!(owner.home.refusal(&candidate).is_some(), "verbatim {}", candidate.display());
        }
        // A junction to the data folder, and one to a harmless folder.
        // mklink reads a `/` as a switch: name both ends with backslashes.
        let native = |path: PathBuf| PathBuf::from(path.to_string_lossy().replace('/', "\\"));
        let junction = |name: &str, target: PathBuf| {
            let link = native(owner.home.path(&format!("BuwizNsh/agents/{name}")));
            let made = Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&link).arg(native(target)).output().expect("cmd");
            assert!(
                made.status.success(),
                "mklink /J: {}{}",
                String::from_utf8_lossy(&made.stdout),
                String::from_utf8_lossy(&made.stderr)
            );
            link
        };
        let to_data = junction("to-data", owner.home.path("BuwizNsh/data"));
        assert!(owner.home.refusal(&to_data.join("bir_data.db")).is_some(), "through a junction");
        assert!(owner.home.refusal(&to_data.join("new.db")).is_some(), "a new file through a junction");
        let scratch = owner.home.make("scratch");
        let to_scratch = junction("to-scratch", scratch);
        assert_eq!(owner.home.refusal(&to_scratch.join("door.db")), None);
        // Another case of every name.
        let upper = PathBuf::from(owner.home.path("BuwizNsh/data/bir_data.db").to_string_lossy().to_uppercase());
        assert!(owner.home.refusal(&upper).is_some(), "{}", upper.display());
        // The 8.3 short name of a protected folder with long names (`GROUPC~1\GROUPD~1.BIR`), where the volume makes
        // short names (not every volume does).
        let long = owner.home.path(GROUP);
        let short = {
            use std::os::windows::ffi::{OsStrExt, OsStringExt};
            let wide: Vec<u16> = long.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            let mut buffer = vec![0u16; 1024];
            // SAFETY: `wide` is NUL-terminated and outlives the call; `buffer` holds the units the call is told of.
            let len = unsafe {
                windows_sys::Win32::Storage::FileSystem::GetShortPathNameW(
                    wide.as_ptr(),
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                )
            } as usize;
            PathBuf::from(std::ffi::OsString::from_wide(&buffer[..len.min(buffer.len())]))
        };
        if !short.as_os_str().is_empty() && !short.to_string_lossy().eq_ignore_ascii_case(&long.to_string_lossy()) {
            let spelled = short.join("x.db");
            assert!(owner.home.refusal(&spelled).is_some(), "short name {}", spelled.display());
        }
        // A hard link outside the protected folders to the owner's database, and links to a file that is not the
        // owner's.
        let alias = agents.join("alias.db");
        fs::hard_link(data.join("bir_data.db"), &alias).expect("hard link");
        let why = owner.home.refusal(&alias).expect("hard link refused");
        assert!(why.contains("hard link"), "{why}");
        let mine = agents.join("mine.db");
        fs::write(&mine, b"mine").expect("write");
        fs::hard_link(&mine, agents.join("mine-too.db")).expect("hard link");
        assert_eq!(owner.home.refusal(&mine), None);
        assert_eq!(owner.home.refusal(&agents.join("mine-too.db")), None);
        assert_eq!(owner.home.refusal(&agents.join("door.db")), None);
    }

    /// The protected folders hang off the profile folder Windows records for this process's account, not
    /// `USERPROFILE` (or `HOME`): a process started with either pointing elsewhere still protects the owner's real
    /// ones.
    #[cfg(windows)]
    #[test]
    fn the_protected_folders_ignore_userprofile() {
        let elsewhere = tempfile::tempdir().expect("elsewhere");
        let account = account_home().expect("the account has a profile folder");
        assert!(account.is_absolute(), "{}", account.display());
        temp_env::with_var("USERPROFILE", Some(elsewhere.path()), || {
            temp_env::with_var("HOME", Some(elsewhere.path()), || {
                assert_eq!(account_home().as_deref(), Some(account.as_path()));
                let folders = protected_folders_here().expect("folders");
                assert!(folders.len() >= 3, "{folders:?}");
                for folder in folders {
                    assert!(folder.starts_with(&account), "{}", folder.display());
                    assert!(!folder.starts_with(elsewhere.path()), "{}", folder.display());
                }
            })
        });
    }

    /// In a test environment the NSH app's own data folder (the shipped app's real database) is refused like the
    /// owner's folders, by its default name under home; outside one it is the app's database and opens.
    #[test]
    fn a_test_environment_also_protects_the_apps_own_data_folder() {
        let home = Home::new();
        let own = app_data_folders(home.root.path());
        assert!(!own.is_empty());
        let default = own[0].join("bir_data.db");
        assert!(default.starts_with(home.root.path()), "{}", default.display());
        let in_test = protected_folders_for(home.root.path(), true);
        let shipped = protected_folders_for(home.root.path(), false);
        assert_eq!(shipped, protected_folders(home.root.path()).to_vec(), "outside a test environment: unchanged");
        let why = database_refusal(&default, &in_test).expect("refused in a test environment");
        assert!(why.contains("Refusing to open"), "{why}");
        assert_eq!(database_refusal(&default, &shipped), None, "the shipped app opens its own database");
        // A scratch database and the agents folder are untouched either way.
        assert_eq!(database_refusal(&home.path("BuwizNsh/agents/door.db"), &in_test), None);
    }

    /// The Simulator check of `docs/platforms/ios.md`: a database named under the app container's `Documents/`
    /// survives relaunches and a test-environment run may open it, while the app's own data folder
    /// (`Library/Application Support/dev.goldcoders.bir`) stays refused. On iOS the container is HOME and the guard
    /// verifies (Unix), so nothing else is asked.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn a_test_run_may_open_a_database_in_the_containers_documents() {
        let home = Home::new();
        let in_test = protected_folders_for(home.root.path(), true);
        let stable = home.path("Documents/buwiz-check/bir_data.db");
        assert_eq!(database_refusal(&stable, &in_test), None);
        assert_eq!(unverified_refusal(cfg!(any(unix, windows)), &stable, None), None);
        let own = home.path("Library/Application Support/dev.goldcoders.bir/bir_data.db");
        assert!(database_refusal(&own, &in_test).is_some(), "the app's own folder stays refused");
    }

    /// On Windows the guard verifies now: with nothing named, a scratch database opens and the owner's real data
    /// folder stays refused. (Asking creates nothing.)
    #[cfg(windows)]
    #[test]
    fn here_a_scratch_database_opens_and_the_owners_data_does_not() {
        let profile = account_home().expect("the account has a profile folder");
        let scratch = tempfile::tempdir().expect("scratch");
        temp_env::with_var(DATABASE_PATH_ENV, None::<&str>, || {
            assert_eq!(database_refusal_here(&scratch.path().join("door.db")), None);
            for folder in [r"BuwizNsh\data", r"BuwizNsh\migration"] {
                let owner = profile.join(folder).join("bir_data.db");
                let why = database_refusal_here(&owner).unwrap_or_else(|| panic!("{} opened", owner.display()));
                assert!(why.contains("the owner's data"), "{why}");
            }
        });
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

    /// A system with neither Unix's nor Windows' lookups cannot be verified, so no database opens unless the caller
    /// named exactly that file. The rule is pure: this runs it as such a system would.
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
        // Where the lookups exist (Unix, Windows) the rule never applies.
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
            // A test environment (`EBIR_TEST_ENV`, as CI sets it) adds the app's data folders.
            assert_eq!(
                folders,
                protected_folders_for(&account, crate::door::test_environment())
            );
            for folder in protected_folders(&account) {
                assert!(folder.starts_with(&account), "{}", folder.display());
            }
            for folder in folders {
                assert!(
                    !folder.starts_with(elsewhere.path()),
                    "{}",
                    folder.display()
                );
            }
        });
    }
}
