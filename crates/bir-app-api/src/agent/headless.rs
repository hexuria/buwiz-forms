//! Long-running headless AgentHost (`bir-headless serve`).
//!
//! Mirrors gpui-agent `apps/todo-headless`: clap `serve` (default) / `status` /
//! `shutdown` / `logs`, `from_env` + `spawn_mailbox_named`, no GPU window. Opens
//! [`bir_core::db::default_database_path`] (or `BIR_DATABASE_PATH` in CI), not
//! `Database::open_ephemeral()`. Starts the same in-process SFTP submission cron as shell GUI.
//! There is no protocol `Op::Yield`. Exclusivity is TCP bind + live-DB owner
//! lock. `serve --wait` (also `bir-headless --wait`) polls until both are free.
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
use super::headless_daemon;
use super::request_log;
use super::session::Session;

/// Headless BIR daemon: app domain logic, no GPUI / GPU window.
///
/// Bind policy comes from `from_env` (`GPUI_AGENT=1`, loopback default).
#[derive(Parser, Debug)]
#[command(name = "bir-headless")]
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
        eprintln!("--detach is only valid with `serve` (example: bir-headless serve --detach)");
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

/// Open the live (or `BIR_DATABASE_PATH`) SQLCipher database. Never ephemeral.
///
/// Uses [`db::open_serve_database`] (`Database::open` + exclusive owner lock),
/// not `open_or_recreate`: a daemon must not quarantine a taxpayer DB.
pub fn open_serve_database() -> Result<(Database, std::path::PathBuf), String> {
    db::open_serve_database().map_err(format_serve_open_error)
}

fn format_serve_open_error(error: db::DbError) -> String {
    if matches!(error, db::DbError::LiveDatabaseInUse(_)) {
        error.to_string()
    } else {
        format!(
            "failed to open live database {}: {error} \
             (bir-headless will not quarantine or recreate this file; \
             shell GUI uses open_or_recreate on the same default path)",
            db::app_database_path().display()
        )
    }
}

pub fn bind_failure_message(addr: SocketAddr, error: &std::io::Error) -> String {
    if error.kind() == ErrorKind::AddrInUse {
        format!(
            "gpui-agent bind {addr} is in use. Quit shell GUI (in-process mailbox) \
             or this `bir-headless` instance, or set GPUI_AGENT_ADDR. Two AgentHosts \
             must not share a bind (GUI embed vs headless). \
             `bir-headless serve --wait` polls until the port and live-DB lock are free."
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
    match db::try_acquire_live_owner_lock(&db::app_database_path()) {
        Ok(lock) => drop(lock),
        Err(db::DbError::LiveDatabaseInUse(_)) => {
            return Ok(Some(ServeBlocker::DatabaseInUse));
        }
        Err(error) => {
            return Err(format!(
                "failed to probe live DB lock {}: {error}",
                db::app_database_path().display()
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
                return Err(db::DbError::LiveDatabaseInUse(db::app_database_path()).to_string());
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
    let config = match from_env() {
        Ok(Some(config)) => config,
        Ok(None) => {
            eprintln!(
                "bir-headless serve is an automation host. Start it with GPUI_AGENT=1.\n\
                 Example:\n\
                 export GPUI_AGENT=1\n\
                 export GPUI_AGENT_TOKEN=dev-secret\n\
                 export GPUI_AGENT_ADDR=127.0.0.1:17421\n\
                 cargo run --locked --bin bir-headless --features agent -- serve\n\
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
    let using_path_override =
        std::env::var_os("BIR_DATABASE_PATH").is_some_and(|value| !value.is_empty());
    if live_database_token_required(token_set, using_path_override) {
        eprintln!(
            "refusing to open the live app database ({}) without GPUI_AGENT_TOKEN.\n\
             export GPUI_AGENT_TOKEN=... (never log the value). \
             CI may set BIR_DATABASE_PATH to a temp file.",
            db::default_database_path().display()
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
        eprintln!("failed to resolve bir-headless executable: {error}");
        ExitCode::from(2)
    })?;
    match headless_daemon::spawn_detached_serve(&exe, wait) {
        Ok(info) => {
            print!("{}", headless_daemon::format_detach_report(&info));
            eprintln!(
                "detached. follow with: bir-headless logs --follow   (log {})",
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
                eprintln!("waiting for live DB owner lock …");
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(error) => {
                eprintln!("{error}");
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
                "gpui-agent bind {addr} accepted no connections. Quit shell GUI                  or the previous headless process and retry.                  `bir-headless serve --wait` polls until a handshake succeeds."
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
                eprintln!("Failed to initialize Tokio runtime for headless background tasks");
            }
        });

        if let Err(error) = headless_daemon::write_current_pid() {
            eprintln!("pid file: {error}");
        }
        let _pid_guard = ServePidGuard;
        let _discovery_guard = match gpui_agent::discovery::publish_current(addr, APP) {
            Ok(published) => Some(published),
            Err(error) => {
                eprintln!("gpui-agent discovery record not written: {error}");
                None
            }
        };

        eprintln!("gpui-agent listening on {addr} (platform=headless, app=bir-shell)");
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
            "database: {} (default_database_path unless BIR_DATABASE_PATH; exclusive owner lock; not ephemeral)",
            path.display()
        );
        eprintln!(
            "BIR: no GPU · screenshot_unavailable · form.print errors · in-process SFTP cron. \
             Single bind + single live-DB owner. `serve --wait` resumes after GUI quit. \
             No Op::Yield. launchd KeepAlive is optional."
        );

        while !shutdown.load(std::sync::atomic::Ordering::SeqCst) {
            for posted in mailbox.take() {
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
            thread::sleep(Duration::from_millis(50));
        }
        if let Ok(guard) = db.lock()
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
            gpui_agent::discovery::resolve_id(&root, "bir-headless")
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
        assert!(Cli::try_parse_from(["bir-headless"]).is_ok());
        for command in ["serve", "status", "shutdown", "logs"] {
            assert!(
                Cli::try_parse_from(["bir-headless", command]).is_ok(),
                "{command}"
            );
        }
    }

    #[test]
    fn parses_wait_detach_and_log_options() {
        assert!(Cli::try_parse_from(["bir-headless", "--wait"]).is_ok());
        assert!(Cli::try_parse_from(["bir-headless", "serve", "--detach"]).is_ok());
        assert!(Cli::try_parse_from(["bir-headless", "logs", "--tail", "20"]).is_ok());
    }
}
