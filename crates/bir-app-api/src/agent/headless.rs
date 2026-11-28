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
        eprintln!("--detach is only valid with `serve` (example: bir serve --detach)");
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
    serve_database_path_and_account().map(|(path, _)| path)
}

/// [`serve_database_path`] plus the stored sign-in its one read saw — #3: the read that picks
/// the room is the read the daemon captures its expected owner from, and it happens inside
/// serve's retry loop, so a `--wait` retry never opens the new account's room under the old
/// account's expectation. Single-room and dev modes carry the same read: the daemon's
/// sign-in check means the same there.
fn serve_database_path_and_account() -> Result<
    (
        std::path::PathBuf,
        Option<bir_core::cloud_account::StoredAccount>,
    ),
    String,
> {
    if let Some(why) = ignored_override_refusal(db::ignored_database_path().as_deref()) {
        return Err(why);
    }
    let stored = bir_core::cloud_account::load_stored_account_shared()
        .ok()
        .flatten();
    if let Some(named) = db::single_room_database() {
        return Ok((named, stored));
    }
    if let Some(dev) = bir_core::dev_mode::dev_database_path() {
        return Ok((dev, stored));
    }
    serve_database_for(&bir_core::rooms::Layout::here(), stored.as_ref()).map(|path| (path, stored))
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
            // R1(c): serve is a separate process that never runs the claim's start(). While the
            // record is at Copied or past it but not yet HashChecked — the move's hash evidence
            // still at stake (the orchestrator's widened ruling) — opening the room file would
            // migrate its bytes and close the move unfinished, so the room does not answer here.
            // The smaller fix of the two the reviewer offered: refuse, rather than running start()
            // in serve.
            if let Err(message) =
                bir_core::legacy_claim::room_move_evidence_clear(layout, &room.room_id)
            {
                return Err(format!("{message}; serve refuses the room"));
            }
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
    open_serve_database_with_account().map(|(opened, path, _)| (opened, path))
}

/// [`open_serve_database`] plus the stored sign-in of the read that picked the room (#3).
fn open_serve_database_with_account() -> Result<
    (
        Database,
        std::path::PathBuf,
        Option<bir_core::cloud_account::StoredAccount>,
    ),
    String,
> {
    let (path, stored) = serve_database_path_and_account()?;
    db::open_serve_database_at(&path)
        .map(|(opened, opened_path)| (opened, opened_path, stored))
        .map_err(|error| format_serve_open_error(&path, error))
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
                            eprintln!("waiting for bind {addr} …")
                        }
                        ServeBlocker::DatabaseInUse => {
                            eprintln!("waiting for live DB owner lock …")
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
        eprintln!("{why}");
        return Err(ExitCode::from(2));
    }
    let config = match from_env() {
        Ok(Some(config)) => config,
        Ok(None) => {
            eprintln!(
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
            eprintln!("refusing to start automation: {error}");
            return Err(ExitCode::from(2));
        }
    };

    let token_set = config.token.is_some();
    // Single-room mode only where it is allowed (accounts-and-lock I3): in a release build an explicit path is
    // ignored, so the signed-in account's room is the live database and the token is required.
    let using_path_override = db::single_room_database().is_some();
    if live_database_token_required(token_set, using_path_override) {
        eprintln!(
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
            eprintln!("{}", headless_daemon::already_running_message(pid));
            return Err(ExitCode::from(2));
        }
        if let Err(error) = wait_until_unblocked(config.addr, false) {
            eprintln!("{error}");
            return Err(ExitCode::from(if error.contains("already open") {
                1
            } else {
                2
            }));
        }
    }

    let exe = std::env::current_exe().map_err(|error| {
        eprintln!("failed to resolve the bir executable: {error}");
        ExitCode::from(2)
    })?;
    match headless_daemon::spawn_detached_serve(&exe, wait) {
        Ok(info) => {
            print!("{}", headless_daemon::format_detach_report(&info));
            eprintln!(
                "detached. follow with: bir logs --follow   (log {})",
                info.log_path.display()
            );
            Ok(())
        }
        Err(error) => {
            eprintln!("{error}");
            Err(ExitCode::from(2))
        }
    }
}

fn logs(follow: bool, tail: Option<usize>) -> Result<(), ExitCode> {
    let path = headless_daemon::log_path();
    if !path.exists() {
        eprintln!("{}", headless_daemon::missing_log_message(&path));
        return Err(ExitCode::from(1));
    }
    let stop = std::sync::atomic::AtomicBool::new(false);
    headless_daemon::dump_or_follow(&path, follow, tail, &stop, &mut std::io::stdout()).map_err(
        |error| {
            eprintln!("failed to read {}: {error}", path.display());
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

pub(crate) const ACCOUNT_CHANGED: &str = "the signed-in account changed; start bir serve again";

type HeadlessRoomSlot = Arc<Mutex<Option<Arc<Mutex<Database>>>>>;

fn stored_owner_and_server() -> Result<Option<(bir_core::account_session::OwnerKey, String)>, String>
{
    // #4: the shared read holds the account-session lock the save writes under — a same-owner
    // refresh is never read back in its write window — and an unchanged file (by modification
    // time) answers from memory, so a batch costs no keychain call.
    // N7: a read ERROR is not a sign-in state: it rides out as Err, never as nobody.
    let stored =
        bir_core::cloud_account::load_stored_account_shared().map_err(|error| error.to_string())?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    let Some(owner) = bir_core::account_session::OwnerKey::of(&stored) else {
        return Ok(None);
    };
    Ok(Some((owner, stored.server_url)))
}

/// How the per-request account check failed (N7): a definite change — a different owner, or
/// definitely nobody — ends the daemon; a read ERROR refuses only that batch: the file is
/// mid-write or briefly unreadable, and the app treats a read error as "nothing closes", so
/// the daemon keeps running and exits only on a definite different owner or a definite nobody.
/// The open room belongs to the stored sign-in captured when serve started; any owner or
/// server change, including an empty store, ends that room's use.
pub(crate) enum AccountRefusal {
    Changed,
    ReadError(String),
}

impl std::fmt::Display for AccountRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Changed => write!(f, "{ACCOUNT_CHANGED}"),
            Self::ReadError(error) => write!(
                f,
                "the saved sign-in could not be read ({error}); the batch is refused, the daemon keeps running"
            ),
        }
    }
}

fn headless_account_error(
    expected: &Option<(bir_core::account_session::OwnerKey, String)>,
) -> Result<Option<bir_core::account_session::OwnerKey>, AccountRefusal> {
    // The owner is the account: the normalised origin key, so a trailing slash or a server
    // string rewrite resolving to the same origin is not a change (#4). The raw server string
    // rides in the tuple for the capture site only; it is never compared here.
    let read = stored_owner_and_server().map_err(AccountRefusal::ReadError)?;
    let owner = read.map(|(owner, _)| owner);
    if owner != expected.as_ref().map(|(owner, _)| owner.clone()) {
        return Err(AccountRefusal::Changed);
    }
    Ok(owner)
}

/// The daemon cron's room source: the same account check on every tick, with the handle held in
/// a slot so a change can really close the room instead of leaving the cron's Arc behind.
fn headless_room_source(
    slot: HeadlessRoomSlot,
    expected: Option<(bir_core::account_session::OwnerKey, String)>,
    close_flag: Arc<std::sync::atomic::AtomicBool>,
) -> bir_core::background_cron::RoomSource {
    Arc::new(move || {
        let owner = match headless_account_error(&expected) {
            Ok(owner) => owner,
            Err(AccountRefusal::ReadError(_)) => {
                // N7: a read error is not an account change — no room this tick, no close.
                return None;
            }
            Err(AccountRefusal::Changed) => {
                // THE RULING: the first refusal from the worker's tick asks the serve loop for
                // the one shared close and the exit — the daemon does not live on.
                close_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                *slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
                bir_core::account_session::cache_owner(None);
                return None;
            }
        };
        bir_core::account_session::cache_owner(owner.clone());
        slot.lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .map(|db| bir_core::background_cron::RoomTick {
                db,
                key: bir_core::background_cron::fixed_room_key(),
                owner,
                epoch: bir_core::session_epoch::current(),
            })
    })
}

/// THE RULING's one shared close (#613 follow-up): the daemon does not live on after an
/// account change. The FIRST refusal — a batch in the serve loop or the cron worker's tick
/// through `close_flag` — runs this once:
/// - an in-flight send that already holds the old account's token completes under it into the
///   old room, with a bounded wait for the sending keys to drain;
/// - mail and the other generic jobs STOP (`request_shutdown`) instead of running on in the
///   old room;
/// - every handle to the room drops — the host Session's Arc too — so the exclusive owner
///   lock is released and the room can be opened again;
/// - the pid file, the discovery record and the port belong to the serve loop's guards and
///   listener: setting `shutdown` ends the loop and they drop with it, and the process exits.
/// How one batch ended (the check runs before EACH request — #1, the unit-testable heart of
/// N2: the check is a parameter, so the test flips it between request 1 and request 2 of one
/// batch and watches the rest be refused).
pub(crate) enum BatchVerdict {
    Answered,
    RefusedFatal,
    RefusedReadError,
    Quit,
}

pub(crate) fn process_batch(
    batch: Vec<gpui_agent::mailbox::MailboxRequest>,
    host: &Arc<Mutex<Session>>,
    account_close: &Arc<std::sync::atomic::AtomicBool>,
    check: &dyn Fn() -> Result<Option<bir_core::account_session::OwnerKey>, AccountRefusal>,
) -> BatchVerdict {
    let mut posts = batch.into_iter();
    while let Some(posted) = posts.next() {
        let refusal = if account_close.load(std::sync::atomic::Ordering::SeqCst) {
            Some(AccountRefusal::Changed)
        } else {
            check().err()
        };
        if let Some(refusal) = refusal {
            let fatal = matches!(refusal, AccountRefusal::Changed);
            let message = refusal.to_string();
            // the client that triggered the refusal still gets its clear error first,
            // and every later request of this batch is refused the same way
            let id = posted.request.id.clone();
            posted.reply(gpui_agent::Response::err(id, message.clone()));
            for rest in posts.by_ref() {
                let id = rest.request.id.clone();
                rest.reply(gpui_agent::Response::err(id, message.clone()));
            }
            return if fatal {
                BatchVerdict::RefusedFatal
            } else {
                BatchVerdict::RefusedReadError
            };
        }
        let req = posted.request.clone();
        let mut guard = host.lock().expect("host");
        let resp = request_log::handle_request_logged(&mut *guard, req, None, None);
        let stop = guard.should_quit();
        drop(guard);
        posted.reply(resp);
        if stop {
            return BatchVerdict::Quit;
        }
    }
    BatchVerdict::Answered
}

fn close_for_account_change(
    host: &Arc<Mutex<Session>>,
    room_slot: &HeadlessRoomSlot,
    shutdown: &std::sync::atomic::AtomicBool,
) {
    // 1. bounded wait: the sending keys of THIS room drain. A send holding the old account's
    //    token finishes into the old room; it is never cut off mid-flight here.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let busy = bir_core::background_cron::get_active_jobs()
            .lock()
            .map(|jobs| {
                jobs.iter().any(|key| {
                    bir_core::background_cron::room_of_job_key(key)
                        == bir_core::background_cron::fixed_room_key()
                })
            })
            .unwrap_or(false);
        if !busy || std::time::Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    // N14: the line says what already happened, so it prints only now that the close is
    // running and the wait for the in-flight send has ended — never before the 30 s wait.
    eprintln!("the signed-in account changed; bir serve has stopped");
    // 2. mail and the generic jobs stop with the room.
    bir_core::background_cron::request_shutdown();
    // 3. every handle to the room drops.
    host.lock().expect("host").rebuild();
    *room_slot
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    bir_core::account_session::cache_owner(None);
    // 4. the guards and the listener end with the serve loop.
    shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn serve(wait: bool) -> Result<(), ExitCode> {
    bir_core::account_session::fill_owner_cache();
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
        let (opened, path, opened_account) = match open_serve_database_with_account() {
            Ok(opened) => opened,
            Err(error) if wait && error.contains("already open") => {
                eprintln!("waiting for live DB owner lock …");
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(error) => {
                eprintln!("{error}");
                return Err(ExitCode::from(1));
            }
        };
        // #3: the expectation is the same read that opened this room, captured here in the
        // retry loop — a `--wait` retry re-reads and can never open the new account's room
        // still expecting the old one.
        let expected_account = opened_account.and_then(|stored| {
            bir_core::account_session::OwnerKey::of(&stored).map(|owner| (owner, stored.server_url))
        });

        let db = Arc::new(Mutex::new(opened));
        let room_slot: HeadlessRoomSlot = Arc::new(Mutex::new(Some(db.clone())));
        let mut session = Session::headless();
        session.set_db(db.clone(), None);
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
                eprintln!("waiting for bind {} …", config.addr);
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(error) => {
                eprintln!("{}", bind_failure_message(config.addr, &error));
                return Err(ExitCode::from(2));
            }
        };

        if !listener_accepts(addr) {
            drop(host);
            drop(db);
            abandon_deaf_listener(addr, &shutdown);
            if wait {
                eprintln!("waiting for bind {addr} …");
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            eprintln!(
                "gpui-agent bind {addr} accepted no connections. Quit shell GUI                  or the previous headless process and retry.                  `bir serve --wait` polls until a handshake succeeds."
            );
            return Err(ExitCode::from(2));
        }

        let account_close = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cron_source = headless_room_source(
            room_slot.clone(),
            expected_account.clone(),
            account_close.clone(),
        );
        thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async move {
                    bir_core::background_cron::start_cron_jobs(cron_source).await;
                });
            } else {
                eprintln!("Failed to initialize Tokio runtime for headless background tasks");
            }
        });
        drop(db);

        if let Err(error) = headless_daemon::write_current_pid() {
            eprintln!("pid file: {error}");
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
                eprintln!("gpui-agent discovery record not written: {error}");
                None
            }
        };

        eprintln!("gpui-agent listening on {addr} (platform=headless, app=bir)");
        eprintln!(
            "log file: {} (override with {})",
            headless_daemon::log_path().display(),
            headless_daemon::LOG_ENV
        );
        eprintln!(
            "opt-in: GPUI_AGENT=1 · bind via from_env · protocol v2 HMAC · GPUI_AGENT_TOKEN required unless GPUI_AGENT_INSECURE_NO_TOKEN=1"
        );
        if token_set {
            eprintln!(
                "auth: required (GPUI_AGENT_TOKEN set; CLI/MCP send protocol v2 HMAC, never the raw token)"
            );
        } else {
            eprintln!(
                "auth: none (GPUI_AGENT_INSECURE_NO_TOKEN=1 demo only; recipe/MCP still need a client token)"
            );
        }
        if request_log::log_requests_enabled() {
            eprintln!(
                "request log: {} (id/op/invoke/ok; token never logged)",
                request_log::LOG_REQUESTS_ENV
            );
        }
        eprintln!("delivery: semantic only (virtual_unavailable — no GPUI event pipeline)");
        eprintln!(
            "database: {} (the saved sign-in's room, the legacy file before rooms, or BIR_DATABASE_PATH in a debug or test run; exclusive owner lock; not ephemeral)",
            path.display()
        );
        eprintln!(
            "BIR: no GPU · screenshot_unavailable · form.print errors · in-process SFTP cron. \
             Single bind + single live-DB owner. `serve --wait` resumes after GUI quit. \
             No Op::Yield. launchd KeepAlive is optional."
        );

        // Poll fast while a client is sending steps back to back (a recipe
        // run), and fall back to the idle interval once it goes quiet.
        let mut read_error_streak = 0u32;
        let mut last_request = None::<std::time::Instant>;
        while !shutdown.load(std::sync::atomic::Ordering::SeqCst) {
            // THE RULING: the first refusal from the worker's tick — or right here below —
            // runs the one shared close and the daemon exits.
            if account_close.load(std::sync::atomic::Ordering::SeqCst) {
                close_for_account_change(&host, &room_slot, &shutdown);
                // N13: an account-change exit is a failure, never a clean zero.
                return Err(ExitCode::from(3));
            }
            let batch = mailbox.take();
            if !batch.is_empty() {
                let check = || headless_account_error(&expected_account);
                match process_batch(batch, &host, &account_close, &check) {
                    BatchVerdict::Answered => {}
                    BatchVerdict::RefusedFatal => {
                        close_for_account_change(&host, &room_slot, &shutdown);
                        // N13: an account-change exit is a failure, never a clean zero.
                        return Err(ExitCode::from(3));
                    }
                    BatchVerdict::RefusedReadError => {
                        // N7: a read error refuses only this batch; the daemon keeps running —
                        // but a read error that never clears is nobody's sign-in in effect:
                        // the backstop exits after N consecutive refused batches.
                        read_error_streak += 1;
                        if read_error_streak >= 10 {
                            eprintln!(
                                "the saved sign-in cannot be read for {} batches in a row; bir serve stops",
                                read_error_streak
                            );
                            close_for_account_change(&host, &room_slot, &shutdown);
                            return Err(ExitCode::from(3));
                        }
                    }
                    BatchVerdict::Quit => break,
                }
                if read_error_streak > 0 && headless_account_error(&expected_account).is_ok() {
                    read_error_streak = 0;
                }
            }
            if host.lock().expect("host").should_quit() {
                break;
            }
            thread::sleep(drain_interval(last_request.map(|at| at.elapsed())));
        }
        if let Some(db) = room_slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            && let Ok(guard) = db.lock()
            && let Err(error) = guard.checkpoint()
        {
            eprintln!("wal checkpoint on shutdown: {error}");
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
            eprintln!("invalid GPUI_AGENT_ADDR: {error}");
            ExitCode::from(2)
        })?;
    let token = std::env::var("GPUI_AGENT_TOKEN")
        .ok()
        .filter(|value| !value.is_empty());
    let allow_remote = gpui_agent::security::truthy_env("GPUI_AGENT_ALLOW_REMOTE");
    gpui_agent::authorize_client(addr, token.as_deref(), allow_remote).map_err(|error| {
        eprintln!("refusing agent address: {error}");
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
        eprintln!("status failed: {error}");
        ExitCode::from(1)
    })?;
    println!("{}", serde_json::to_string(&resp).expect("hello json"));
    Ok(())
}

fn shutdown() -> Result<(), ExitCode> {
    let mut client = connect_running()?;
    client.expect_ok(Op::Shutdown).map_err(|error| {
        eprintln!("shutdown failed: {error}");
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

    fn stored_for(server_url: &str, user_id: &str) -> bir_core::cloud_account::StoredAccount {
        bir_core::cloud_account::StoredAccount {
            server_url: server_url.to_owned(),
            access_token: format!("access-{user_id}"),
            refresh_token: format!("refresh-{user_id}"),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: user_id.to_owned(),
            email: Some(format!("{user_id}@example.test")),
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        }
    }

    fn reviewed_profile() -> bir_core::profile::TaxpayerProfile {
        use bir_core::profile::{IncomeTaxElection, TaxElectionHistory};
        let mut profile: bir_core::profile::TaxpayerProfile =
            serde_json::from_value(serde_json::json!({
                "id": null,
                "full_name": "Headless Queue Taxpayer",
                "tin": {"segment1":"123","segment2":"456","segment3":"789","branch":"000"},
                "rdo_code": "018",
                "line_of_business": "Retail",
                "registered_address": "Manila",
                "zip_code": "1000",
                "phone": "09123456789",
                "email": "headless@example.test",
                "default_form_type": "2551Qv2018",
                "taxpayer_type": "Individual",
                "business_start_date": "2010-01-01"
            }))
            .unwrap();
        profile.tax_elections.push(TaxElectionHistory {
            taxable_year: 2024,
            election: IncomeTaxElection::GraduatedUnspecified,
            elected_at: chrono::NaiveDateTime::default(),
            source_form: "2551Qv2018".to_owned(),
        });
        profile.ensure_profile_version_ledger();
        profile
    }

    /// F4 through the daemon's production room source and cron loop: A's queued return is sent
    /// while A remains signed in (the positive control); when the disk says B, the same source
    /// refuses, empties the room slot, and the owner lock can be taken again.
    #[test]
    fn headless_room_source_sends_for_a_then_refuses_b_and_closes_the_room() {
        use bir_core::forms::{FilingStatus, Form2551QDraft, form_2551q::Item13Election};
        let dir = tempfile::tempdir().unwrap();
        let account_path = dir.path().join("account.json");
        let db_path = dir.path().join("bir_data.db");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        temp_env::with_vars(
            [
                (
                    bir_core::cloud_account::ENV_ACCOUNT_FILE,
                    Some(account_path.to_str().unwrap().to_owned()),
                ),
                ("BIR_SFTP_DRY_RUN", Some("1".to_owned())),
                ("BIR_RELAY_URL", None),
                ("BUWIZ_RELAY_SEND", None),
            ],
            || {
                runtime.block_on(async {
                    let stored_a = stored_for("http://localhost:3008", "user-a");
                    bir_core::account_session::save_stored(&stored_a).unwrap();
                    let owner_a = bir_core::account_session::OwnerKey::of(&stored_a).unwrap();
                    let expected = Some((owner_a.clone(), stored_a.server_url.clone()));
                    let (opened, _) = bir_core::db::open_serve_database_at(&db_path).unwrap();
                    let db = std::sync::Arc::new(std::sync::Mutex::new(opened));
                    let profile = reviewed_profile();
                    let queued_tin;
                    {
                        let guard = db.lock().unwrap();
                        guard.save_profile(profile.clone()).unwrap();
                        let mut draft =
                            Form2551QDraft::new_from_effective_profile(&profile, 2024, 1);
                        queued_tin = draft.tin.clone();
                        draft.item_13_election = Item13Election::Graduated;
                        draft.transition_to_queued().unwrap();
                        guard.save_queued_2551q_draft_and_election(&draft).unwrap();
                    }
                    let slot: super::HeadlessRoomSlot =
                        std::sync::Arc::new(std::sync::Mutex::new(Some(db.clone())));
                    let source = super::headless_room_source(
                        slot.clone(),
                        expected.clone(),
                        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                    );
                    let stop = bir_core::background_cron::CronStop::new();
                    let worker = tokio::spawn(bir_core::background_cron::run_cron_jobs(
                        source.clone(),
                        stop.clone(),
                    ));
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                    loop {
                        let status = db
                            .lock()
                            .unwrap()
                            .get_2551q_draft(&queued_tin, 2024, 1)
                            .unwrap()
                            .unwrap()
                            .status;
                        if status == FilingStatus::Submitted {
                            break;
                        }
                        assert!(
                            std::time::Instant::now() < deadline,
                            "the headless positive control did not send: {status:?}"
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                    }
                    assert_eq!(
                        bir_core::account_session::stored_owner(),
                        Some(owner_a.clone()),
                        "the daemon source did not fill the owner cache for the account path"
                    );

                    let stored_b = stored_for("http://localhost:3008", "user-b");
                    bir_core::account_session::save_stored(&stored_b).unwrap();
                    assert!(
                        matches!(
                            super::headless_account_error(&expected).unwrap_err(),
                            super::AccountRefusal::Changed
                        ),
                        "a different owner is the definite account change"
                    );
                    assert!(
                        source().is_none(),
                        "the changed account still had a room tick"
                    );
                    assert!(
                        slot.lock().unwrap().is_none(),
                        "the room slot stayed open after the account changed"
                    );
                    stop.stop();
                    worker.await.unwrap();
                    drop(db);
                    bir_core::db::open_serve_database_at(&db_path)
                        .expect("the room's owner lock was still held after the close");
                    bir_core::account_session::cache_owner(None);
                });
            },
        );
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

    /// #5, THE PROOF: through serve()'s own start-up and one batch, on the token-fetching
    /// transport of #613's F6 tests — never the dry run, whose route never reaches
    /// AccountTokens or SEND_CONTEXT. A queued → sent: the transport's open really asked the
    /// process-wide AccountTokens for a token under the worker's SEND_CONTEXT (the counters
    /// count it), and the store counted the send. Then B on disk → the batch's client still
    /// gets its clear error, the daemon does the one shared close and exits: the room's lock
    /// is free (the test opens the room itself) and the pid file is gone. The nobody-on-disk
    /// case: the same. Positive control: with A still on disk the same batch is answered,
    /// never refused. STORED_OWNER is restored, the transport restored, and the global cron
    /// worker stopped at the end.
    #[test]
    fn serve_exits_on_the_account_change_through_the_relay_route() {
        use bir_core::forms::{Form2551QDraft, form_2551q::Item13Election};
        use gpui_agent::Op;
        let dir = tempfile::tempdir().unwrap();
        let account_path = dir.path().join("account.json");
        let db_path = dir.path().join("bir_data.db");

        // the room before serve opens it: one reviewed profile, one queued 2551Q
        {
            let (opened, _) = bir_core::db::open_serve_database_at(&db_path).unwrap();
            let profile = reviewed_profile();
            let mut draft = Form2551QDraft::new_from_effective_profile(&profile, 2024, 1);
            draft.item_13_election = Item13Election::Graduated;
            draft.transition_to_queued().unwrap();
            opened.save_profile(profile).unwrap();
            opened.save_queued_2551q_draft_and_election(&draft).unwrap();
        }

        let account_a = stored_for("http://localhost:3008", "user-a");
        let account_b = stored_for("http://localhost:3008", "user-b");
        // the port is held until the env is set, then freed for serve to bind
        let holder = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let serve_addr = holder.local_addr().unwrap().to_string();
        let env: Vec<(&str, Option<String>)> = vec![
            (
                bir_core::cloud_account::ENV_ACCOUNT_FILE,
                Some(account_path.to_str().unwrap().to_owned()),
            ),
            ("EBIR_TEST_ENV", Some("1".into())),
            ("BUWIZ_NO_KEYCHAIN", Some("1".into())),
            (
                "BIR_DATABASE_PATH",
                Some(db_path.to_str().unwrap().to_owned()),
            ),
            ("GPUI_AGENT", Some("1".into())),
            ("GPUI_AGENT_TOKEN", Some("proof-token".into())),
            ("GPUI_AGENT_ADDR", Some(serve_addr.clone())),
            (
                "BIR_HEADLESS_LOG",
                Some(dir.path().join("serve.log").to_str().unwrap().to_owned()),
            ),
            (
                "BIR_HEADLESS_PID",
                Some(dir.path().join("serve.pid").to_str().unwrap().to_owned()),
            ),
            (
                "GPUI_AGENT_RUNTIME_DIR",
                Some(dir.path().join("rt").to_str().unwrap().to_owned()),
            ),
        ];
        temp_env::with_vars(env, || {
            drop(holder);
            let _guards = ProofGuards::arm();
            bir_core::background_cron::use_token_fetching_transport(true);
            bir_core::background_cron::reset_token_transport_counts();
            bir_core::account_session::save_stored(&account_a).unwrap();
            let outcome = std::sync::Arc::new(std::sync::Mutex::new(None));
            let outcome_in_thread = outcome.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                tx.send(()).unwrap();
                *outcome_in_thread.lock().unwrap() = Some(super::serve(false));
            });
            rx.recv().unwrap();
            let pid_path = dir.path().join("serve.pid");
            let log = || std::fs::read_to_string(dir.path().join("serve.log")).unwrap_or_default();

            // POSITIVE CONTROL: with A on disk the batch is answered, never refused.
            let mut client = gpui_agent::AgentClient::connect(serve_addr.parse().unwrap())
                .with_timeout(std::time::Duration::from_secs(3))
                .with_token("proof-token".to_owned());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            loop {
                if client.expect_ok(Op::Hello).is_ok() {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "the daemon never answered; log: {}",
                    log()
                );
                std::thread::sleep(std::time::Duration::from_millis(150));
            }

            // A queued → SENT: the token fetch happened (the open counted it) and the send
            // was stored — never the dry-run route, which reaches neither.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
            loop {
                let (opens, stores) = bir_core::background_cron::token_transport_counts();
                if opens > 0 && stores > 0 {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "no token-fetching send happened (opens {opens}, stores {stores}); log: {}",
                    log()
                );
                std::thread::sleep(std::time::Duration::from_millis(300));
            }

            // B on disk: the next batch is refused with the clear error, then the daemon
            // does the one shared close and exits.
            bir_core::account_session::save_stored(&account_b).unwrap();
            // no pause: the batch must reach the serve loop before the cron's own tick can
            // close the room first — the batch path's clear error is what is asserted here
            // Either refusal order is the ruling working: the batch reaches the serve loop
            // first and is answered with the clear error, or the cron's own tick closes the
            // room first and the batch dies with the connection. What must NEVER happen is
            // the batch answered Ok — the change is always seen.
            // N1: STICKY, not resettable — B's save precedes the first Hello, so ANY Ok at
            // all fails, whether a later Err follows it or not. Either refusal order is the
            // ruling working: the clear error, or the close's own disconnect.
            for _ in 0..20 {
                match client.expect_ok(Op::Hello) {
                    Ok(_) => panic!("a batch under B is never answered Ok"),
                    Err(error) => {
                        assert!(
                            error.to_string().contains("the signed-in account changed")
                                || error.to_string().contains("no reply")
                                || error.to_string().contains("connect")
                                || error.to_string().contains("closed"),
                            "the clear error or the close's own disconnect, got: {error}"
                        );
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            wait_for_serve_exit(&outcome);
            assert!(
                !pid_path.exists(),
                "the pid file is gone once the daemon has stopped"
            );
            // the room's lock is FREE: the test opens the room itself
            drop(
                bir_core::db::open_serve_database_at(&db_path).unwrap_or_else(|error| {
                    panic!("the room must open again after the close: {error}")
                }),
            );

            // nobody on disk: the same. A fresh serve under B, then the file goes away.
            bir_core::account_session::save_stored(&account_b).unwrap();
            let outcome2 = std::sync::Arc::new(std::sync::Mutex::new(None));
            let outcome2_in_thread = outcome2.clone();
            let (tx2, rx2) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                tx2.send(()).unwrap();
                *outcome2_in_thread.lock().unwrap() = Some(super::serve(false));
            });
            rx2.recv().unwrap();
            let mut client2 = gpui_agent::AgentClient::connect(serve_addr.parse().unwrap())
                .with_timeout(std::time::Duration::from_secs(3))
                .with_token("proof-token".to_owned());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            loop {
                if client2.expect_ok(Op::Hello).is_ok() {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "the second daemon never answered"
                );
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            // sign out entirely: nobody on disk
            let _ = std::fs::remove_file(&account_path);
            bir_core::account_session::cache_owner(None);
            // N1: sticky here too — nobody on disk precedes the first Hello, so ANY Ok fails.
            for _ in 0..20 {
                match client2.expect_ok(Op::Hello) {
                    Ok(_) => panic!("a batch with nobody on disk is never answered Ok"),
                    Err(error) => {
                        assert!(
                            error.to_string().contains("the signed-in account changed")
                                || error.to_string().contains("no reply")
                                || error.to_string().contains("connect")
                                || error.to_string().contains("closed"),
                            "the clear error or the close's own disconnect, got: {error}"
                        );
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            wait_for_serve_exit(&outcome2);
            drop(
                bir_core::db::open_serve_database_at(&db_path)
                    .unwrap_or_else(|error| panic!("the room must open again: {error}")),
            );

            // the Drop guards restore STORED_OWNER, the transport, and the cron worker even
            // when an assertion panics — the release workflows' cargo test runs these too.
        });
    }

    /// N8: the proof's global-state restores as Drop guards, so a panicking assertion can
    /// never leak them into the next test of the same process.
    struct ProofGuards;
    impl ProofGuards {
        fn arm() -> Self {
            ProofGuards
        }
    }
    impl Drop for ProofGuards {
        fn drop(&mut self) {
            bir_core::account_session::cache_owner(None);
            bir_core::background_cron::use_token_fetching_transport(false);
            bir_core::background_cron::request_shutdown();
        }
    }

    fn wait_for_serve_exit(
        outcome: &std::sync::Arc<std::sync::Mutex<Option<Result<(), std::process::ExitCode>>>>,
    ) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
        loop {
            if outcome.lock().unwrap().is_some() {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the daemon did not exit after the account change"
            );
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    /// N2, the proof that discriminates: ONE batch — the mailbox's own posts — worked through
    /// by the production loop body, with the account check FLIPPING between request 1 and
    /// request 2. Request 1 is answered; request 2 and every later request are refused with
    /// the clear error and the verdict is the fatal close. With the check at batch level the
    /// whole batch is answered — the test fails. (The pipeline-shaped proof could never batch:
    /// the driver answers one request per connection, so the second loop iteration never ran.)
    #[test]
    fn a_batch_is_checked_before_each_request() {
        use gpui_agent::{Op, protocol::Request};
        use std::sync::{Arc, Mutex};
        let mailbox = gpui_agent::AgentMailbox::new();
        let rx1 = mailbox.push(Request::new("r1", Op::Hello));
        let rx2 = mailbox.push(Request::new("r2", Op::Hello));
        let rx3 = mailbox.push(Request::new("r3", Op::Hello));
        let batch = mailbox.take();
        assert_eq!(
            batch.len(),
            3,
            "positive control: one batch of three requests"
        );
        let host = Arc::new(Mutex::new(crate::agent::session::Session::new(
            crate::agent::session::UiQueue::discarding(),
        )));
        let close = Arc::new(std::sync::atomic::AtomicBool::new(false));
        // the check flips after the first call: request 1 passes, request 2 is the change
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let check = || {
            if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                Ok(None)
            } else {
                Err(super::AccountRefusal::Changed)
            }
        };
        let verdict = super::process_batch(batch, &host, &close, &check);
        assert!(
            matches!(verdict, super::BatchVerdict::RefusedFatal),
            "the batch ends in the fatal close"
        );
        assert!(rx1.recv().unwrap().ok, "request 1 is answered");
        let err2 = rx2.recv().unwrap();
        assert!(!err2.ok, "request 2 is refused");
        assert!(
            err2.error
                .unwrap_or_default()
                .contains("the signed-in account changed"),
            "the refusal is the clear error"
        );
        assert!(
            !rx3.recv().unwrap().ok,
            "request 3 — the rest of the batch — is refused"
        );
    }

    /// N7: a read ERROR of the sign-in file is not an account change. The batch is refused
    /// with the read error, and the daemon KEEPS RUNNING — where the old code read
    /// `.ok().flatten()`, counted the error as nobody-on-disk and exited. Positive control:
    /// once the file reads again, the next batch is answered Ok.
    #[test]
    #[cfg(unix)]
    fn a_read_error_refuses_the_batch_but_the_daemon_keeps_running() {
        use gpui_agent::Op;
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            eprintln!("skipped as root: chmod 000 is ignored for root");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let account_path = dir.path().join("account.json");
        let db_path = dir.path().join("bir_data.db");
        let account_a = stored_for("http://localhost:3008", "user-a");
        let holder = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let serve_addr = holder.local_addr().unwrap().to_string();
        let env: Vec<(&str, Option<String>)> = vec![
            (
                bir_core::cloud_account::ENV_ACCOUNT_FILE,
                Some(account_path.to_str().unwrap().to_owned()),
            ),
            ("EBIR_TEST_ENV", Some("1".into())),
            ("BUWIZ_NO_KEYCHAIN", Some("1".into())),
            (
                "BIR_DATABASE_PATH",
                Some(db_path.to_str().unwrap().to_owned()),
            ),
            ("GPUI_AGENT", Some("1".into())),
            ("GPUI_AGENT_TOKEN", Some("proof-token".into())),
            ("GPUI_AGENT_ADDR", Some(serve_addr.clone())),
            (
                "BIR_HEADLESS_LOG",
                Some(dir.path().join("serve.log").to_str().unwrap().to_owned()),
            ),
            (
                "BIR_HEADLESS_PID",
                Some(dir.path().join("serve.pid").to_str().unwrap().to_owned()),
            ),
            (
                "GPUI_AGENT_RUNTIME_DIR",
                Some(dir.path().join("rt").to_str().unwrap().to_owned()),
            ),
        ];
        temp_env::with_vars(env, || {
            drop(holder);
            let _guards = ProofGuards::arm();
            bir_core::account_session::save_stored(&account_a).unwrap();
            let outcome = std::sync::Arc::new(std::sync::Mutex::new(None));
            let outcome_in_thread = outcome.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                tx.send(()).unwrap();
                *outcome_in_thread.lock().unwrap() = Some(super::serve(false));
            });
            rx.recv().unwrap();
            let mut client = gpui_agent::AgentClient::connect(serve_addr.parse().unwrap())
                .with_timeout(std::time::Duration::from_secs(3))
                .with_token("proof-token".to_owned());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            loop {
                if client.expect_ok(Op::Hello).is_ok() {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "the daemon never answered"
                );
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            // the file is there but cannot be READ. A trailing newline first: the
            // mtime/length/inode cache must MISS or the cached owner answers and the file
            // is never read at all.
            let mut bytes = std::fs::read(&account_path).unwrap();
            bytes.push(b'\n');
            std::fs::write(&account_path, &bytes).unwrap();
            std::fs::set_permissions(&account_path, std::fs::Permissions::from_mode(0o000))
                .unwrap();
            let error = client
                .expect_ok(Op::Hello)
                .expect_err("the batch is refused")
                .to_string();
            assert!(
                error.contains("could not be read"),
                "the refusal is the read error, got: {error}"
            );
            // restore: the daemon is still running and answers again — it did NOT exit
            std::fs::set_permissions(&account_path, std::fs::Permissions::from_mode(0o600))
                .unwrap();
            bir_core::account_session::save_stored(&account_a).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                if client.expect_ok(Op::Hello).is_ok() {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "positive control failed: the daemon must keep running after a read error"
                );
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            let _ = client.expect_ok(Op::Shutdown);
            // the shutdown is real: the serve thread ends, never a slept-for assumption
            wait_for_serve_exit(&outcome);
        });
    }

    /// #4: the account check compares the OwnerKey — the normalized origin — not the raw
    /// server_url string: a trailing slash appearing or going in a same-account token refresh
    /// is not an account change, and the daemon does not exit for nothing.
    /// Positive control: a different user_id still errors.
    #[test]
    fn the_account_check_compares_the_owner_not_the_raw_url() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.json");
        temp_env::with_var(
            bir_core::cloud_account::ENV_ACCOUNT_FILE,
            Some(path.to_str().unwrap()),
            || {
                let slashed = stored_for("http://localhost:3008/", "user-1");
                bir_core::account_session::save_stored(&slashed).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&slashed).unwrap();
                // The expected tuple as a start under the unslashed spelling captured it.
                let unslashed = stored_for("http://localhost:3008", "user-1");
                let expected = Some((
                    bir_core::account_session::OwnerKey::of(&unslashed).unwrap(),
                    unslashed.server_url.clone(),
                ));
                assert_eq!(owner, expected.clone().unwrap().0, "the owner keys are one");
                assert!(
                    super::headless_account_error(&expected).is_ok(),
                    "a trailing slash is not an account change"
                );
                // Positive control: another user really is a change.
                let other = stored_for("http://localhost:3008", "user-2");
                bir_core::account_session::save_stored(&other).unwrap();
                assert!(
                    super::headless_account_error(&expected).is_err(),
                    "another account errors"
                );
            },
        );
    }

    /// The ruling, test (a): a reader that sees ZERO BYTES — the torn read of the in-place
    /// PermissionDenied fallback, common on Windows — gets a read error and the daemon keeps
    /// running; it does not exit as if signed out. The empty file here is exactly what the
    /// torn fallback write exposes mid-write.
    #[test]
    fn zero_bytes_is_a_read_error_and_the_daemon_keeps_running() {
        use gpui_agent::Op;
        let dir = tempfile::tempdir().unwrap();
        let account_path = dir.path().join("account.json");
        let db_path = dir.path().join("bir_data.db");
        let account_a = stored_for("http://localhost:3008", "user-a");
        let holder = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let serve_addr = holder.local_addr().unwrap().to_string();
        let env: Vec<(&str, Option<String>)> = vec![
            (
                bir_core::cloud_account::ENV_ACCOUNT_FILE,
                Some(account_path.to_str().unwrap().to_owned()),
            ),
            ("EBIR_TEST_ENV", Some("1".into())),
            ("BUWIZ_NO_KEYCHAIN", Some("1".into())),
            (
                "BIR_DATABASE_PATH",
                Some(db_path.to_str().unwrap().to_owned()),
            ),
            ("GPUI_AGENT", Some("1".into())),
            ("GPUI_AGENT_TOKEN", Some("proof-token".into())),
            ("GPUI_AGENT_ADDR", Some(serve_addr.clone())),
            (
                "BIR_HEADLESS_LOG",
                Some(dir.path().join("serve.log").to_str().unwrap().to_owned()),
            ),
            (
                "BIR_HEADLESS_PID",
                Some(dir.path().join("serve.pid").to_str().unwrap().to_owned()),
            ),
            (
                "GPUI_AGENT_RUNTIME_DIR",
                Some(dir.path().join("rt").to_str().unwrap().to_owned()),
            ),
        ];
        temp_env::with_vars(env, || {
            drop(holder);
            let _guards = ProofGuards::arm();
            bir_core::account_session::save_stored(&account_a).unwrap();
            let outcome = std::sync::Arc::new(std::sync::Mutex::new(None));
            let outcome_in_thread = outcome.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                tx.send(()).unwrap();
                *outcome_in_thread.lock().unwrap() = Some(super::serve(false));
            });
            rx.recv().unwrap();
            let mut client = gpui_agent::AgentClient::connect(serve_addr.parse().unwrap())
                .with_timeout(std::time::Duration::from_secs(3))
                .with_token("proof-token".to_owned());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            loop {
                if client.expect_ok(Op::Hello).is_ok() {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "the daemon never answered"
                );
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            // zero bytes — the torn read of the in-place fallback. The trailing-newline cache
            // miss of the chmod test is not needed: empty differs in length from any save.
            std::fs::write(&account_path, "").unwrap();
            assert!(
                client.expect_ok(Op::Hello).is_err(),
                "the batch over a torn read is refused, not answered"
            );
            // restore: the daemon is still running and answers again — it did NOT exit
            bir_core::account_session::save_stored(&account_a).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                if client.expect_ok(Op::Hello).is_ok() {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "positive control failed: the daemon must keep running after a torn read"
                );
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            let _ = client.expect_ok(Op::Shutdown);
            wait_for_serve_exit(&outcome);
        });
    }

    /// The ruling, test (c): an old-format EMPTY file left by an earlier build is a read
    /// error, and the backstop — ten consecutive read-error batches — ends the daemon, so it
    /// is not held refusing for ever.
    #[test]
    fn an_old_empty_file_is_a_read_error_and_the_backstop_ends_it() {
        use gpui_agent::Op;
        let dir = tempfile::tempdir().unwrap();
        let account_path = dir.path().join("account.json");
        let db_path = dir.path().join("bir_data.db");
        let account_a = stored_for("http://localhost:3008", "user-a");
        let holder = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let serve_addr = holder.local_addr().unwrap().to_string();
        let env: Vec<(&str, Option<String>)> = vec![
            (
                bir_core::cloud_account::ENV_ACCOUNT_FILE,
                Some(account_path.to_str().unwrap().to_owned()),
            ),
            ("EBIR_TEST_ENV", Some("1".into())),
            ("BUWIZ_NO_KEYCHAIN", Some("1".into())),
            (
                "BIR_DATABASE_PATH",
                Some(db_path.to_str().unwrap().to_owned()),
            ),
            ("GPUI_AGENT", Some("1".into())),
            ("GPUI_AGENT_TOKEN", Some("proof-token".into())),
            ("GPUI_AGENT_ADDR", Some(serve_addr.clone())),
            (
                "BIR_HEADLESS_LOG",
                Some(dir.path().join("serve.log").to_str().unwrap().to_owned()),
            ),
            (
                "BIR_HEADLESS_PID",
                Some(dir.path().join("serve.pid").to_str().unwrap().to_owned()),
            ),
            (
                "GPUI_AGENT_RUNTIME_DIR",
                Some(dir.path().join("rt").to_str().unwrap().to_owned()),
            ),
        ];
        temp_env::with_vars(env, || {
            drop(holder);
            let _guards = ProofGuards::arm();
            bir_core::account_session::save_stored(&account_a).unwrap();
            let outcome = std::sync::Arc::new(std::sync::Mutex::new(None));
            let outcome_in_thread = outcome.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                tx.send(()).unwrap();
                *outcome_in_thread.lock().unwrap() = Some(super::serve(false));
            });
            rx.recv().unwrap();
            let mut client = gpui_agent::AgentClient::connect(serve_addr.parse().unwrap())
                .with_timeout(std::time::Duration::from_secs(3))
                .with_token("proof-token".to_owned());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            loop {
                if client.expect_ok(Op::Hello).is_ok() {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "the daemon never answered"
                );
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            // the old-format empty file: a read error now, every batch refused
            std::fs::write(&account_path, "").unwrap();
            for batch in 1..=10 {
                assert!(
                    client.expect_ok(Op::Hello).is_err(),
                    "batch {batch} over the empty file is refused"
                );
            }
            // the backstop ends the daemon — never a slept-for assumption
            wait_for_serve_exit(&outcome);
            let end = outcome.lock().unwrap();
            assert!(
                end.is_some(),
                "the backstop ended the daemon after the consecutive read errors"
            );
        });
    }

    /// The daemon half of (b): the sentinel file — what a sign-out leaves when the remove
    /// fails — is a definite nobody, and the daemon exits AT ONCE (no backstop count, no
    /// refusal streak): one batch is enough.
    #[test]
    fn the_sentinel_file_exits_the_daemon_at_once() {
        use gpui_agent::Op;
        let dir = tempfile::tempdir().unwrap();
        let account_path = dir.path().join("account.json");
        let db_path = dir.path().join("bir_data.db");
        let account_a = stored_for("http://localhost:3008", "user-a");
        let holder = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let serve_addr = holder.local_addr().unwrap().to_string();
        let env: Vec<(&str, Option<String>)> = vec![
            (
                bir_core::cloud_account::ENV_ACCOUNT_FILE,
                Some(account_path.to_str().unwrap().to_owned()),
            ),
            ("EBIR_TEST_ENV", Some("1".into())),
            ("BUWIZ_NO_KEYCHAIN", Some("1".into())),
            (
                "BIR_DATABASE_PATH",
                Some(db_path.to_str().unwrap().to_owned()),
            ),
            ("GPUI_AGENT", Some("1".into())),
            ("GPUI_AGENT_TOKEN", Some("proof-token".into())),
            ("GPUI_AGENT_ADDR", Some(serve_addr.clone())),
            (
                "BIR_HEADLESS_LOG",
                Some(dir.path().join("serve.log").to_str().unwrap().to_owned()),
            ),
            (
                "BIR_HEADLESS_PID",
                Some(dir.path().join("serve.pid").to_str().unwrap().to_owned()),
            ),
            (
                "GPUI_AGENT_RUNTIME_DIR",
                Some(dir.path().join("rt").to_str().unwrap().to_owned()),
            ),
        ];
        temp_env::with_vars(env, || {
            drop(holder);
            let _guards = ProofGuards::arm();
            bir_core::account_session::save_stored(&account_a).unwrap();
            let outcome = std::sync::Arc::new(std::sync::Mutex::new(None));
            let outcome_in_thread = outcome.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                tx.send(()).unwrap();
                *outcome_in_thread.lock().unwrap() = Some(super::serve(false));
            });
            rx.recv().unwrap();
            let mut client = gpui_agent::AgentClient::connect(serve_addr.parse().unwrap())
                .with_timeout(std::time::Duration::from_secs(3))
                .with_token("proof-token".to_owned());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            loop {
                if client.expect_ok(Op::Hello).is_ok() {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "the daemon never answered"
                );
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            // the sign-out's sentinel: a definite nobody — one batch ends the daemon
            // the sentinel literal pinned, not the constant: the format is a contract with
            // every build that ever wrote it, so the test spells it out
            std::fs::write(&account_path, "{\"signed_out\":true}\n").unwrap();
            let _ = client.expect_ok(Op::Hello);
            wait_for_serve_exit(&outcome);
            let end = outcome.lock().unwrap();
            assert!(
                end.is_some(),
                "the sentinel ended the daemon at once — one batch, no backstop count"
            );
        });
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
