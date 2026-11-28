//! Long-running headless AgentHost (`bir serve`).
//!
//! Mirrors gpui-agent `apps/todo-headless`: clap `serve` (default) / `status` /
//! `shutdown` / `logs`, `from_env` + `spawn_mailbox_named`, no GPU window. Opens
//! the room of the account in the saved sign-in, or the legacy file while no room
//! exists ([`serve_database_path`]; or `BIR_DATABASE_PATH` in a debug or test
//! run), not `Database::open_ephemeral()`. Starts the same in-process SFTP submission cron as shell GUI.
//! There is no protocol `Op::Yield`. Exclusivity is TCP bind + live-DB owner
//! lock. `serve --wait` (also `bir --wait`) polls until both are free.
//! `serve --detach` backgrounds the daemon (Docker `-d`); `logs --follow`
//! attaches to the logfile.

use std::io::ErrorKind;
use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use bir_core::db::{self, Database};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use gpui_agent::DEFAULT_ADDR_STR;
use gpui_agent::client::AgentClient;
use gpui_agent::mailbox::AgentMailbox;
use gpui_agent::protocol::Op;
use gpui_agent::security::from_env;
use gpui_agent::server::spawn_mailbox_named;

use super::APP;

macro_rules! redacted_eprintln {
    ($($arg:tt)*) => {
        bir_core::logging::redacted_stderr(&format!($($arg)*))
    };
}

/// The executable the discovery record names. The binary is `bir`, as is the NSH app's record, so the daemon keeps
/// the old name here: `bir-mcp` reads it as headless mode and `connect_running` finds the daemon by it.
const RECORD_EXECUTABLE: &str = "bir-headless";
use super::headless_daemon;
use super::request_log;
use super::session::Session;

/// Headless BIR daemon: app domain logic, no GPUI / GPU window.
///
/// Bind policy comes from `from_env` (`GPUI_AGENT=1`, loopback default).
#[derive(Parser, Debug)]
#[command(name = "bir")]
struct Cli {
    /// Poll until bind and the live-DB owner lock are free, then serve.
    /// Without this flag, a busy bind or lock exits immediately.
    #[arg(long, global = true)]
    wait: bool,
    /// Background `serve` (Docker `-d`). Print pid + log path and exit.
    /// Only valid with `serve`. Follow later with `logs --follow`.
    #[arg(long, global = true)]
    detach: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
enum Command {
    /// Bind the agent protocol and serve until shutdown (default).
    Serve,
    /// Print hello/ready from a running daemon.
    Status,
    /// Ask a running daemon to exit.
    Shutdown,
    /// Dump or follow the daemon logfile (`docker logs` / `tail -f`).
    Logs {
        /// Follow new lines (`docker logs -f` / `tail -f`). Ctrl-C exits.
        #[arg(long, short = 'f')]
        follow: bool,
        /// Show only the last N lines (like `docker logs --tail N`).
        #[arg(long, value_name = "N")]
        tail: Option<usize>,
    },
}

pub fn run() -> ExitCode {
    // First, before any record: without it a real `serve --detach` panic went raw into bir-headless.log, TINs and all.
    bir_core::logging::install(bir_core::logging::InstallOptions {
        stdout: true,
        developer_mode: bir_core::logging::developer_mode_enabled(),
        default_filter: "warn,bir_app_api=info,bir_core=info,js=warn",
    });
    // Test-only hook, armed only under EBIR_TEST_ENV: the detached-daemon logging proof needs a real panic
    // carrying a TIN inside the real child after it is serving, and nothing in production sets this variable.
    if std::env::var_os("EBIR_TEST_ENV").is_some()
        && let Ok(message) = std::env::var("BIR_TEST_PANIC_ON_START")
    {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            panic!("{message}");
        });
    }
    bir_core::notification::enable_desktop_delivery();
    dotenvy::dotenv().ok();
    match run_cli() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn executable_stem(argv0: &str) -> &str {
    let stem = std::path::Path::new(argv0)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("bir");
    if stem.is_empty() { "bir" } else { stem }
}

fn parse_cli() -> Cli {
    let mut command = Cli::command();
    if let Some(argv0) = std::env::args().next() {
        command.set_bin_name(executable_stem(&argv0));
    }
    let matches = command.get_matches();
    Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
}

fn run_cli() -> Result<(), ExitCode> {
    let cli = parse_cli();
    let command = cli.command.unwrap_or(Command::Serve);
    if cli.detach && !matches!(command, Command::Serve) {
        redacted_eprintln!("--detach is only valid with `serve` (example: bir serve --detach)");
        return Err(ExitCode::from(2));
    }
    match command {
        Command::Serve if cli.detach => detach_serve(cli.wait),
        Command::Serve => serve(cli.wait),
        Command::Status => status(),
        Command::Shutdown => shutdown(),
        Command::Logs { follow, tail } => logs(follow, tail),
    }
}

/// What `bir-headless serve` answers when there is no saved sign-in, or no room for it.
pub const NOT_SIGNED_IN: &str = "Sign in in the eBIRForms app first.";

/// The database `bir-headless serve` opens (accounts-and-lock PLAN §A.2, decided), in
/// `db::app_database_path`'s order:
/// - where single-room mode is allowed (a debug build or a test environment), the file an explicit
///   `BIR_DATABASE_PATH` names, as before; a debug build's separate dev database;
/// - the room of the account in the saved sign-in, found through `device.db`'s `rooms`;
/// - while no room exists yet (no `rooms/` folder: every install until A3a), the legacy `data_dir()/bir_data.db`
///   when that file is there, so A1 changes nothing for a headless run.
///
/// Otherwise it refuses with [`NOT_SIGNED_IN`]. It never creates `data_dir()/bir_data.db` or a room, and it never
/// makes `device.db`.
pub fn serve_database_path() -> Result<std::path::PathBuf, String> {
    if let Some(why) = ignored_override_refusal(db::ignored_database_path().as_deref()) {
        return Err(why);
    }
    if let Some(named) = db::single_room_database() {
        return Ok(named);
    }
    if let Some(dev) = bir_core::dev_mode::dev_database_path() {
        return Ok(dev);
    }
    let stored = bir_core::cloud_account::load_stored_account()
        .ok()
        .flatten();
    serve_database_for(&bir_core::rooms::Layout::here(), stored.as_ref())
}

/// Why `serve` refuses a database it was named but would not obey (`BIR_DATABASE_PATH` or the in-process name in a
/// release build outside a test environment, accounts-and-lock I3): rather than run, and its cron, on the live data the
/// caller did not ask for. `None` when nothing is named, or the name is obeyed.
pub(crate) fn ignored_override_refusal(ignored: Option<&std::path::Path>) -> Option<String> {
    ignored.map(|path| {
        format!(
            "refusing to start: BIR_DATABASE_PATH ({}) is ignored in a release build outside a test environment, \
             so serve would open the live data instead; unset it, or use a debug build or EBIR_TEST_ENV=1.",
            path.display()
        )
    })
}

/// [`serve_database_path`] after single-room mode: the signed-in room under `layout`, else the legacy file while no
/// room exists, else [`NOT_SIGNED_IN`] (or why the device store could not be read, once rooms exist).
pub(crate) fn serve_database_for(
    layout: &bir_core::rooms::Layout,
    stored: Option<&bir_core::cloud_account::StoredAccount>,
) -> Result<std::path::PathBuf, String> {
    let room = signed_in_room_database(layout, stored);
    if room.is_ok() || layout.has_rooms() {
        return room;
    }
    let legacy = layout.legacy_database();
    if legacy.is_file() {
        Ok(legacy)
    } else {
        Err(NOT_SIGNED_IN.to_owned())
    }
}

/// The room file of `stored`'s account under `layout`, if `rooms` has it and the file is there.
pub(crate) fn signed_in_room_database(
    layout: &bir_core::rooms::Layout,
    stored: Option<&bir_core::cloud_account::StoredAccount>,
) -> Result<std::path::PathBuf, String> {
    let stored = stored.ok_or_else(|| NOT_SIGNED_IN.to_owned())?;
    let store = bir_core::rooms::DeviceStore::open_existing(layout)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| NOT_SIGNED_IN.to_owned())?;
    match bir_core::rooms::room_for(Some(&store), &stored.server_url, &stored.user_id) {
        Ok(bir_core::rooms::RoomLookup::Known(room)) => {
            let path = layout.room_database(&room.room_id);
            if path.is_file() {
                Ok(path)
            } else {
                Err(NOT_SIGNED_IN.to_owned())
            }
        }
        Ok(bir_core::rooms::RoomLookup::New { .. }) | Err(_) => Err(NOT_SIGNED_IN.to_owned()),
    }
}

/// Open the database [`serve_database_path`] names. Never ephemeral.
///
/// Uses [`db::open_serve_database_at`] (`Database::open` + exclusive owner lock),
/// not `open_or_recreate`: a daemon must not quarantine a taxpayer DB.
pub fn open_serve_database() -> Result<(Database, std::path::PathBuf), String> {
    let path = serve_database_path()?;
    db::open_serve_database_at(&path).map_err(|error| format_serve_open_error(&path, error))
}

fn format_serve_open_error(path: &std::path::Path, error: db::DbError) -> String {
    if matches!(error, db::DbError::LiveDatabaseInUse(_)) {
        error.to_string()
    } else {
        format!(
            "failed to open live database {}: {error} \
             (bir will not quarantine or recreate this file; \
             shell GUI uses open_or_recreate on the same default path)",
            path.display()
        )
    }
}

pub fn bind_failure_message(addr: SocketAddr, error: &std::io::Error) -> String {
    if error.kind() == ErrorKind::AddrInUse {
        format!(
            "gpui-agent bind {addr} is in use. Quit shell GUI (in-process mailbox) \
             or this `bir` instance, or set GPUI_AGENT_ADDR. Two AgentHosts \
             must not share a bind (GUI embed vs headless). \
             `bir serve --wait` polls until the port and live-DB lock are free."
        )
    } else {
        format!("gpui-agent failed to bind {addr}: {error}")
    }
}

/// Why `serve` cannot start yet. No protocol `Op::Yield`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServeBlocker {
    BindInUse,
    DatabaseInUse,
}

/// Occupy `addr` without `listen`.
///
/// `TcpListener::bind` listens. A client can finish the TCP handshake into
/// that backlog before anything calls `accept`. On Windows, dropping the
/// listener without `accept` still lets a later socket bind and log that it
/// is listening, but new agent handshakes on the port then fail. `serve
/// --wait` polls this probe while a client may already be connecting, so the
/// probe must not listen. Connection attempts get refused; the port is still
/// busy for a real bind.
struct PortProbe {
    _socket: socket2::Socket,
}

impl PortProbe {
    fn bind(addr: SocketAddr) -> std::io::Result<Self> {
        let domain = match addr {
            SocketAddr::V4(_) => socket2::Domain::IPV4,
            SocketAddr::V6(_) => socket2::Domain::IPV6,
        };
        let socket =
            socket2::Socket::new(domain, socket2::Type::STREAM, Some(socket2::Protocol::TCP))?;
        if addr.is_ipv6() {
            socket.set_only_v6(true)?;
        }
        socket.set_reuse_address(false)?;
        socket.bind(&socket2::SockAddr::from(addr))?;
        Ok(Self { _socket: socket })
    }
}

/// Probe bind then the live-DB sidecar lock. Does not keep either.
pub fn serve_blocker(addr: SocketAddr) -> Result<Option<ServeBlocker>, String> {
    match PortProbe::bind(addr) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::AddrInUse => {
            return Ok(Some(ServeBlocker::BindInUse));
        }
        Err(error) => return Err(format!("gpui-agent failed to bind {addr}: {error}")),
    }
    let path = serve_database_path()?;
    match db::try_acquire_live_owner_lock(&path) {
        Ok(lock) => drop(lock),
        Err(db::DbError::LiveDatabaseInUse(_)) => {
            return Ok(Some(ServeBlocker::DatabaseInUse));
        }
        Err(error) => {
            return Err(format!(
                "failed to probe live DB lock {}: {error}",
                path.display()
            ));
        }
    }
    Ok(None)
}

/// If `wait` is false, the first blocker is an error. If true, poll until clear.
pub fn wait_until_unblocked(addr: SocketAddr, wait: bool) -> Result<(), String> {
    let mut last: Option<ServeBlocker> = None;
    let mut last_print = Instant::now() - Duration::from_secs(10);
    loop {
        match serve_blocker(addr)? {
            None => return Ok(()),
            Some(blocker) if wait => {
                if last != Some(blocker) || last_print.elapsed() >= Duration::from_secs(2) {
                    match blocker {
                        ServeBlocker::BindInUse => {
                            redacted_eprintln!("waiting for bind {addr} …")
                        }
                        ServeBlocker::DatabaseInUse => {
                            redacted_eprintln!("waiting for live DB owner lock …")
                        }
                    }
                    last = Some(blocker);
                    last_print = Instant::now();
                }
                thread::sleep(Duration::from_millis(200));
            }
            Some(ServeBlocker::BindInUse) => {
                return Err(bind_failure_message(
                    addr,
                    &std::io::Error::from(ErrorKind::AddrInUse),
                ));
            }
            Some(ServeBlocker::DatabaseInUse) => {
                return Err(db::DbError::LiveDatabaseInUse(serve_database_path()?).to_string());
            }
        }
    }
}

/// Live `default_database_path()` (no `BIR_DATABASE_PATH`) requires a token.
/// `from_env` already default-denies untokened binds; this still refuses
/// `GPUI_AGENT_INSECURE_NO_TOKEN=1` against the live taxpayer file.
pub fn live_database_token_required(token_set: bool, using_path_override: bool) -> bool {
    !using_path_override && !token_set
}

fn load_serve_config() -> Result<gpui_agent::security::AgentConfig, ExitCode> {
    // First: a named database this build would not obey opens nothing (accounts-and-lock I3).
    if let Some(why) = ignored_override_refusal(db::ignored_database_path().as_deref()) {
        redacted_eprintln!("{why}");
        return Err(ExitCode::from(2));
    }
    let config = match from_env() {
        Ok(Some(config)) => config,
        Ok(None) => {
            redacted_eprintln!(
                "bir serve is an automation host. Start it with GPUI_AGENT=1.\n\
                 Example:\n\
                 export GPUI_AGENT=1\n\
                 export GPUI_AGENT_TOKEN=dev-secret\n\
                 export GPUI_AGENT_ADDR=127.0.0.1:17421\n\
                 cargo run --locked -p bir-cli -- serve\n\
                 (GPUI_AGENT_INSECURE_NO_TOKEN=1 is demo-only; never against the live DB)"
            );
            return Err(ExitCode::from(2));
        }
        Err(error) => {
            redacted_eprintln!("refusing to start automation: {error}");
            return Err(ExitCode::from(2));
        }
    };

    let token_set = config.token.is_some();
    // Single-room mode only where it is allowed (accounts-and-lock I3): in a release build an explicit path is
    // ignored, so the signed-in account's room is the live database and the token is required.
    let using_path_override = db::single_room_database().is_some();
    if live_database_token_required(token_set, using_path_override) {
        redacted_eprintln!(
            "refusing to open the live app database (the signed-in account's room) without GPUI_AGENT_TOKEN.\n\
             export GPUI_AGENT_TOKEN=... (never log the value). \
             CI may set BIR_DATABASE_PATH to a temp file."
        );
        return Err(ExitCode::from(2));
    }
    Ok(config)
}

fn detach_serve(wait: bool) -> Result<(), ExitCode> {
    let config = load_serve_config()?;
    if !wait {
        if let Some(pid) = headless_daemon::live_pid(&headless_daemon::pid_path()) {
            redacted_eprintln!("{}", headless_daemon::already_running_message(pid));
            return Err(ExitCode::from(2));
        }
        if let Err(error) = wait_until_unblocked(config.addr, false) {
            redacted_eprintln!("{error}");
            return Err(ExitCode::from(if error.contains("already open") {
                1
            } else {
                2
            }));
        }
    }

    let exe = std::env::current_exe().map_err(|error| {
        redacted_eprintln!("failed to resolve the bir executable: {error}");
        ExitCode::from(2)
    })?;
    match headless_daemon::spawn_detached_serve(&exe, wait) {
        Ok(info) => {
            print!("{}", headless_daemon::format_detach_report(&info));
            redacted_eprintln!(
                "detached. follow with: bir logs --follow   (log {})",
                info.log_path.display()
            );
            Ok(())
        }
        Err(error) => {
            redacted_eprintln!("{error}");
            Err(ExitCode::from(2))
        }
    }
}

fn logs(follow: bool, tail: Option<usize>) -> Result<(), ExitCode> {
    let path = headless_daemon::log_path();
    if !path.exists() {
        redacted_eprintln!("{}", headless_daemon::missing_log_message(&path));
        return Err(ExitCode::from(1));
    }
    let stop = std::sync::atomic::AtomicBool::new(false);
    headless_daemon::dump_or_follow(&path, follow, tail, &stop, &mut std::io::stdout()).map_err(
        |error| {
            redacted_eprintln!("failed to read {}: {error}", path.display());
            ExitCode::from(1)
        },
    )
}

struct ServePidGuard;

impl Drop for ServePidGuard {
    fn drop(&mut self) {
        headless_daemon::remove_pid_file_if_current(&headless_daemon::pid_path());
    }
}

/// True when a loopback client finishes the TCP handshake.
///
/// On Windows, `bind` + `listen` can succeed on a port whose previous
/// listener was closed, while every new handshake still fails. Logging
/// "listening" is not readiness. `serve --wait` drops that socket and
/// binds again instead of publishing a pid for a deaf port.
fn listener_accepts(addr: SocketAddr) -> bool {
    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok()
}

fn abandon_deaf_listener(addr: SocketAddr, shutdown: &std::sync::atomic::AtomicBool) {
    shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if PortProbe::bind(addr).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn serve(wait: bool) -> Result<(), ExitCode> {
    let mut config = load_serve_config()?;
    // `--wait` exists to wait for the port (restart handoff), so it must not
    // fall back to an ephemeral port while the old server still holds it.
    if !wait {
        config.addr = super::resolve_addr(config.addr);
    }
    let token_set = config.token.is_some();

    loop {
        // The real `listen` is the bind check. A throwaway successful bind
        // (what `serve_blocker` does) is closed before that listen. On
        // Windows, closing the previous socket — listening or not — can
        // leave the next listener unable to accept.
        let (opened, path) = match open_serve_database() {
            Ok(opened) => opened,
            Err(error) if wait && error.contains("already open") => {
                redacted_eprintln!("waiting for live DB owner lock …");
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(error) => {
                redacted_eprintln!("{error}");
                return Err(ExitCode::from(1));
            }
        };

        let db = Arc::new(Mutex::new(opened));
        let mut session = Session::headless();
        session.set_db(db.clone());
        let host = Arc::new(Mutex::new(session));
        let mailbox = AgentMailbox::new();
        let (addr, shutdown) = match spawn_mailbox_named(
            config.addr,
            config.token.clone(),
            mailbox.clone(),
            Duration::from_secs(8),
            APP,
        ) {
            Ok(started) => started,
            Err(error) if wait && error.kind() == ErrorKind::AddrInUse => {
                drop(host);
                drop(db);
                redacted_eprintln!("waiting for bind {} …", config.addr);
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(error) => {
                redacted_eprintln!("{}", bind_failure_message(config.addr, &error));
                return Err(ExitCode::from(2));
            }
        };

        if !listener_accepts(addr) {
            drop(host);
            drop(db);
            abandon_deaf_listener(addr, &shutdown);
            if wait {
                redacted_eprintln!("waiting for bind {addr} …");
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            redacted_eprintln!(
                "gpui-agent bind {addr} accepted no connections. Quit shell GUI                  or the previous headless process and retry.                  `bir serve --wait` polls until a handshake succeeds."
            );
            return Err(ExitCode::from(2));
        }

        let cron_db = db.clone();
        thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async move {
                    bir_core::background_cron::start_cron_jobs(cron_db).await;
                });
            } else {
                redacted_eprintln!(
                    "Failed to initialize Tokio runtime for headless background tasks"
                );
            }
        });

        if let Err(error) = headless_daemon::write_current_pid() {
            redacted_eprintln!("pid file: {error}");
        }
        let _pid_guard = ServePidGuard;
        // The serve thread's one write must land first, or it overwrites
        // this record after it is written (see try_start).
        crate::agent::wait_for_serve_record(&addr);
        let identity = gpui_agent::discovery::detect_identity();
        let record = gpui_agent::discovery::HostRecord {
            addr: addr.to_string(),
            pid: std::process::id(),
            app: APP.to_owned(),
            executable: RECORD_EXECUTABLE.to_owned(),
            bundle_id: identity.bundle_id,
            connect_id: identity.connect_id,
        };
        let root = gpui_agent::discovery::runtime_root();
        let _discovery_guard = match crate::agent::publish_record_retrying(&root, &record) {
            Ok(published) => Some(published),
            Err(error) => {
                redacted_eprintln!("gpui-agent discovery record not written: {error}");
                None
            }
        };

        redacted_eprintln!("gpui-agent listening on {addr} (platform=headless, app=bir)");
        redacted_eprintln!(
            "log file: {} (override with {})",
            headless_daemon::log_path().display(),
            headless_daemon::LOG_ENV
        );
        redacted_eprintln!(
            "opt-in: GPUI_AGENT=1 · bind via from_env · protocol v2 HMAC · GPUI_AGENT_TOKEN required unless GPUI_AGENT_INSECURE_NO_TOKEN=1"
        );
        if token_set {
            redacted_eprintln!(
                "auth: required (GPUI_AGENT_TOKEN set; CLI/MCP send protocol v2 HMAC, never the raw token)"
            );
        } else {
            redacted_eprintln!(
                "auth: none (GPUI_AGENT_INSECURE_NO_TOKEN=1 demo only; recipe/MCP still need a client token)"
            );
        }
        if request_log::log_requests_enabled() {
            redacted_eprintln!(
                "request log: {} (id/op/invoke/ok; token never logged)",
                request_log::LOG_REQUESTS_ENV
            );
        }
        redacted_eprintln!(
            "delivery: semantic only (virtual_unavailable — no GPUI event pipeline)"
        );
        redacted_eprintln!(
            "database: {} (the saved sign-in's room, the legacy file before rooms, or BIR_DATABASE_PATH in a debug or test run; exclusive owner lock; not ephemeral)",
            path.display()
        );
        redacted_eprintln!(
            "BIR: no GPU · screenshot_unavailable · form.print errors · in-process SFTP cron. \
             Single bind + single live-DB owner. `serve --wait` resumes after GUI quit. \
             No Op::Yield. launchd KeepAlive is optional."
        );

        // Poll fast while a client is sending steps back to back (a recipe
        // run), and fall back to the idle interval once it goes quiet.
        let mut last_request = None::<std::time::Instant>;
        while !shutdown.load(std::sync::atomic::Ordering::SeqCst) {
            for posted in mailbox.take() {
                last_request = Some(std::time::Instant::now());
                let req = posted.request.clone();
                let mut guard = host.lock().expect("host");
                let resp = request_log::handle_request_logged(&mut *guard, req, None, None);
                let stop = guard.should_quit();
                drop(guard);
                posted.reply(resp);
                if stop {
                    shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
                    break;
                }
            }
            if host.lock().expect("host").should_quit() {
                break;
            }
            thread::sleep(drain_interval(last_request.map(|at| at.elapsed())));
        }
        if let Ok(guard) = db.lock()
            && let Err(error) = guard.checkpoint()
        {
            redacted_eprintln!("wal checkpoint on shutdown: {error}");
        }
        return Ok(());
    }
}

fn connect_running() -> Result<AgentClient, ExitCode> {
    let addr = std::env::var("GPUI_AGENT_ADDR")
        .unwrap_or_else(|_| {
            let root = gpui_agent::discovery::runtime_root();
            gpui_agent::discovery::resolve_id(&root, RECORD_EXECUTABLE)
                .or_else(|_| gpui_agent::discovery::resolve_id(&root, APP))
                .map(|record| record.addr)
                .unwrap_or_else(|_| DEFAULT_ADDR_STR.to_owned())
        })
        .parse()
        .map_err(|error| {
            redacted_eprintln!("invalid GPUI_AGENT_ADDR: {error}");
            ExitCode::from(2)
        })?;
    let token = std::env::var("GPUI_AGENT_TOKEN")
        .ok()
        .filter(|value| !value.is_empty());
    let allow_remote = gpui_agent::security::truthy_env("GPUI_AGENT_ALLOW_REMOTE");
    gpui_agent::authorize_client(addr, token.as_deref(), allow_remote).map_err(|error| {
        redacted_eprintln!("refusing agent address: {error}");
        ExitCode::from(2)
    })?;
    let mut client = AgentClient::connect(addr).with_timeout(Duration::from_secs(3));
    if let Some(token) = token {
        client = client.with_token(token);
    }
    Ok(client)
}

fn status() -> Result<(), ExitCode> {
    let mut client = connect_running()?;
    let resp = client.expect_ok(Op::Hello).map_err(|error| {
        redacted_eprintln!("status failed: {error}");
        ExitCode::from(1)
    })?;
    println!("{}", serde_json::to_string(&resp).expect("hello json"));
    Ok(())
}

fn shutdown() -> Result<(), ExitCode> {
    let mut client = connect_running()?;
    client.expect_ok(Op::Shutdown).map_err(|error| {
        redacted_eprintln!("shutdown failed: {error}");
        ExitCode::from(1)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn parses_default_serve_and_subcommands() {
        assert!(Cli::try_parse_from(["bir"]).is_ok());
        for command in ["serve", "status", "shutdown", "logs"] {
            assert!(Cli::try_parse_from(["bir", command]).is_ok(), "{command}");
        }
    }

    #[test]
    fn parses_wait_detach_and_log_options() {
        assert!(Cli::try_parse_from(["bir", "--wait"]).is_ok());
        assert!(Cli::try_parse_from(["bir", "serve", "--detach"]).is_ok());
        assert!(Cli::try_parse_from(["bir", "logs", "--tail", "20"]).is_ok());
    }

    fn signed_in(server_url: &str, user_id: &str) -> bir_core::cloud_account::StoredAccount {
        bir_core::cloud_account::StoredAccount {
            server_url: server_url.to_owned(),
            access_token: "access".to_owned(),
            refresh_token: "refresh".to_owned(),
            access_expires_at_unix: 0,
            user_id: user_id.to_owned(),
            email: Some("jane@buwiz.com".to_owned()),
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        }
    }

    /// Accounts-and-lock A1 (I3): with no explicit path and no legacy file, `serve` opens only the saved sign-in's
    /// room, found through `rooms`. No sign-in, no device store, no row, or no file: refused with the sentence, and
    /// nothing is made (no `bir_data.db`, no room, no `device.db`).
    #[test]
    fn headless_refuses_without_a_signed_in_room() {
        use super::{NOT_SIGNED_IN, serve_database_for};
        use bir_core::rooms::{DeviceStore, Layout, RoomRecord, room_id_for};
        let folder = tempfile::tempdir().unwrap();
        let layout = Layout::at(folder.path());
        let jane = signed_in("https://api.buwiz.com/", "user-1");
        let nothing_made = |layout: &Layout| {
            assert!(!layout.legacy_database().exists(), "no bir_data.db");
            assert!(!layout.rooms_dir().exists(), "no room");
        };

        assert_eq!(
            serve_database_for(&layout, None).unwrap_err(),
            NOT_SIGNED_IN
        );
        assert_eq!(
            serve_database_for(&layout, Some(&jane)).unwrap_err(),
            NOT_SIGNED_IN
        );
        nothing_made(&layout);
        assert!(!layout.device_db().exists(), "no device.db");

        let store = DeviceStore::open(&layout).unwrap();
        assert_eq!(
            serve_database_for(&layout, Some(&jane)).unwrap_err(),
            NOT_SIGNED_IN
        );
        let room = room_id_for("https://api.buwiz.com", "user-1");
        store
            .add_room(&RoomRecord {
                room_id: room.clone(),
                origin: "https://api.buwiz.com".to_owned(),
                user_id: "user-1".to_owned(),
                email: Some("jane@buwiz.com".to_owned()),
                created_at: "2026-11-26T00:00:00+00:00".to_owned(),
                from_legacy: false,
            })
            .unwrap();
        // The row without its file is still no room to open.
        assert_eq!(
            serve_database_for(&layout, Some(&jane)).unwrap_err(),
            NOT_SIGNED_IN
        );
        nothing_made(&layout);

        layout.create_room_dir(&room).unwrap();
        std::fs::write(layout.room_database(&room), b"").unwrap();
        assert_eq!(
            serve_database_for(&layout, Some(&jane)).unwrap(),
            layout.room_database(&room)
        );
        // Another account, or the same id on another server, has no room here.
        let other = signed_in("https://api.buwiz.com", "user-2");
        assert_eq!(
            serve_database_for(&layout, Some(&other)).unwrap_err(),
            NOT_SIGNED_IN
        );
        let test_server = signed_in("http://127.0.0.1:3008", "user-1");
        assert_eq!(
            serve_database_for(&layout, Some(&test_server)).unwrap_err(),
            NOT_SIGNED_IN
        );
        assert!(!layout.legacy_database().exists(), "no bir_data.db");
    }

    /// Accounts-and-lock A1 (I3): a database named where single-room mode is not allowed (a release build outside a
    /// test environment) is refused with one line, never ignored for the live data. Checked through the rule itself in
    /// every build, and through the process in a release build (`cargo test --release`).
    #[test]
    fn release_serve_refuses_an_ignored_database_path() {
        use super::ignored_override_refusal;
        let named = std::path::Path::new("/tmp/named/bir_data.db");
        let why = ignored_override_refusal(Some(named)).expect("refused");
        assert!(
            why.starts_with(
                "refusing to start: BIR_DATABASE_PATH (/tmp/named/bir_data.db) is ignored"
            ),
            "{why}"
        );
        assert_eq!(why.lines().count(), 1, "one line");
        assert_eq!(ignored_override_refusal(None), None);
        // Here (a debug test build) a named database is obeyed, so nothing is ignored.
        if cfg!(debug_assertions) {
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("bir_data.db");
            temp_env::with_var("BIR_DATABASE_PATH", Some(&path), || {
                assert_eq!(bir_core::db::ignored_database_path(), None);
                assert_eq!(super::serve_database_path().unwrap(), path);
            });
        } else {
            temp_env::with_vars(
                [
                    ("EBIR_TEST_ENV", None),
                    ("BIR_DATABASE_PATH", Some("/tmp/named/bir_data.db")),
                ],
                || {
                    let refused = super::serve_database_path().unwrap_err();
                    assert!(refused.starts_with("refusing to start"), "{refused}");
                },
            );
        }
    }

    /// Accounts-and-lock A1: until a room exists (no `rooms/` folder, every install before A3a), `serve` opens the
    /// legacy `bir_data.db` when it is there, signed in or not, as before A1. It makes nothing. Once any room exists,
    /// the legacy file is never the answer: it may be waiting for another account.
    #[test]
    fn headless_uses_the_legacy_file_before_rooms_exist() {
        use super::{NOT_SIGNED_IN, serve_database_for};
        use bir_core::rooms::{Layout, room_id_for};
        let folder = tempfile::tempdir().unwrap();
        let layout = Layout::at(folder.path());
        std::fs::write(layout.legacy_database(), b"").unwrap();
        let jane = signed_in("https://api.buwiz.com", "user-1");

        assert_eq!(
            serve_database_for(&layout, None).unwrap(),
            layout.legacy_database()
        );
        assert_eq!(
            serve_database_for(&layout, Some(&jane)).unwrap(),
            layout.legacy_database()
        );
        assert!(!layout.device_db().exists(), "no device.db made");
        assert!(!layout.rooms_dir().exists(), "no room made");

        // Another account's room exists: the legacy file is no longer opened.
        layout
            .create_room_dir(&room_id_for("https://api.buwiz.com", "user-2"))
            .unwrap();
        assert_eq!(
            serve_database_for(&layout, Some(&jane)).unwrap_err(),
            NOT_SIGNED_IN
        );
        assert_eq!(
            serve_database_for(&layout, None).unwrap_err(),
            NOT_SIGNED_IN
        );
    }
}

/// How long the serve loop sleeps between mailbox drains, given the time
/// since the last request (`None` before the first one).
///
/// Only the wait between drains changes: requests are still taken and
/// answered one at a time, in arrival order, on the one thread that owns the
/// database, so the system model is unaffected.
fn drain_interval(since_last_request: Option<Duration>) -> Duration {
    super::drain_interval(since_last_request, Duration::from_millis(50))
}

#[cfg(test)]
mod drain_interval_tests {
    use super::drain_interval;
    use std::time::Duration;

    #[test]
    fn idle_until_first_request() {
        assert_eq!(drain_interval(None), Duration::from_millis(50));
    }

    #[test]
    fn fast_while_busy_then_back_to_idle() {
        assert_eq!(
            drain_interval(Some(Duration::ZERO)),
            Duration::from_millis(1)
        );
        assert_eq!(
            drain_interval(Some(Duration::from_millis(499))),
            Duration::from_millis(1)
        );
        assert_eq!(
            drain_interval(Some(Duration::from_millis(500))),
            Duration::from_millis(50)
        );
        assert_eq!(
            drain_interval(Some(Duration::from_secs(60))),
            Duration::from_millis(50)
        );
    }
}
