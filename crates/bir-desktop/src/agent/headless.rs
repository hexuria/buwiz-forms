//! Long-running headless AgentHost (`bir-headless serve`).
//!
//! Mirrors gpui-agent `apps/todo-headless`: clap `serve` (default) / `status` /
//! `shutdown`, `from_env` + `spawn_host`, no GPU window. Opens
//! [`bir_core::db::default_database_path`] (or `BIR_DATABASE_PATH` in CI), not
//! `Database::open_ephemeral()`. Starts the same in-process SFTP submission cron as painted `bir`.
//! There is no protocol `Op::Yield`. Exclusivity is TCP bind + live-DB owner
//! lock. `serve --wait` (also `bir-headless --wait`) polls until both are free.

use std::io::ErrorKind;
use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use bir_core::db::{self, Database};
use clap::{Parser, Subcommand};
use gpui_agent::DEFAULT_ADDR_STR;
use gpui_agent::client::AgentClient;
use gpui_agent::mailbox::AgentMailbox;
use gpui_agent::protocol::{Op, PlatformKind};
use gpui_agent::security::from_env;
use gpui_agent::server::{spawn_host, spawn_mailbox};

use super::host::BirAgentHost;
use super::request_log;

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
}

pub fn run() -> ExitCode {
    match run_cli() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run_cli() -> Result<(), ExitCode> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(cli.wait),
        Command::Status => status(),
        Command::Shutdown => shutdown(),
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
             painted bir uses open_or_recreate on the same default path)",
            db::app_database_path().display()
        )
    }
}

pub fn bind_failure_message(addr: SocketAddr, error: &std::io::Error) -> String {
    if error.kind() == ErrorKind::AddrInUse {
        format!(
            "gpui-agent bind {addr} is in use. Quit painted `bir` (in-process mailbox) \
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

/// Probe bind then the live-DB sidecar lock. Does not keep either.
pub fn serve_blocker(addr: SocketAddr) -> Result<Option<ServeBlocker>, String> {
    match std::net::TcpListener::bind(addr) {
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

pub fn host_for_database(db: Arc<Mutex<Database>>) -> BirAgentHost {
    BirAgentHost::new(PlatformKind::Headless).with_database(db)
}

/// Live `default_database_path()` (no `BIR_DATABASE_PATH`) requires a token.
pub fn live_database_token_required(token_set: bool, using_path_override: bool) -> bool {
    !using_path_override && !token_set
}

fn serve(wait: bool) -> Result<(), ExitCode> {
    let config = match from_env() {
        Ok(Some(config)) => config,
        Ok(None) => {
            eprintln!(
                "bir-headless serve is an automation host. Start it with GPUI_AGENT=1.\n\
                 Example:\n\
                 export GPUI_AGENT=1\n\
                 export GPUI_AGENT_TOKEN=dev-secret\n\
                 export GPUI_AGENT_ADDR=127.0.0.1:17421\n\
                 cargo run --locked --bin bir-headless --features agent -- serve"
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

    loop {
        if let Err(error) = wait_until_unblocked(config.addr, wait) {
            eprintln!("{error}");
            return Err(ExitCode::from(if error.contains("already open") {
                1
            } else {
                2
            }));
        }

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
        let host = Arc::new(Mutex::new(host_for_database(db.clone())));
        let mailbox = AgentMailbox::new();
        let drain_token = config.token.clone();
        let (addr, shutdown) = match spawn_mailbox(
            config.addr,
            config.token.clone(),
            mailbox.clone(),
            Duration::from_secs(8),
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

        eprintln!("gpui-agent listening on {addr} (platform=headless, app=bir-desktop)");
        eprintln!("opt-in: GPUI_AGENT=1 · bind via from_env · protocol v1");
        if token_set {
            eprintln!(
                "auth: required (GPUI_AGENT_TOKEN set; recipe/MCP clients must send the same token)"
            );
        } else {
            eprintln!(
                "auth: none (one-off click/snapshot ok; recipe run and mcp need the same token on host and client)"
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
                let resp =
                    request_log::handle_request_logged(&mut *guard, req, drain_token.as_deref());
                let stop = guard.wants_shutdown();
                drop(guard);
                posted.reply(resp);
                if stop {
                    shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
                    break;
                }
            }
            if host.lock().expect("host").wants_shutdown() {
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
    let addr = match std::env::var("GPUI_AGENT_ADDR") {
        Ok(raw) => raw.parse::<SocketAddr>().map_err(|error| {
            eprintln!("invalid GPUI_AGENT_ADDR: {error}");
            ExitCode::from(2)
        })?,
        Err(_) => DEFAULT_ADDR_STR.parse().expect("default addr"),
    };
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
    use super::*;
    use crate::agent::host::empty_host;
    use crate::agent::ids;
    use clap::Parser;
    use gpui_agent::dispatch::handle_request;
    use gpui_agent::protocol::{Op, Request};
    use serde_json::json;
    use std::net::TcpListener;
    use std::path::PathBuf;

    fn req(op: Op) -> Request {
        Request::new("t", op)
    }

    fn fill_profile_editor(host: &mut BirAgentHost, tin: &str, name: &str) {
        handle_request(
            host,
            req(Op::Invoke {
                name: "profile.create".into(),
                args: json!({}),
            }),
            None,
        );
        for (target, value) in [
            (ids::PROFILE_TIN, tin),
            (ids::PROFILE_NAME, name),
            (ids::PROFILE_RDO, "018"),
            (ids::PROFILE_LOB, "Retail"),
            (ids::PROFILE_ADDRESS, "Manila"),
            (ids::PROFILE_ZIP, "1000"),
            (ids::PROFILE_PHONE, "09170000000"),
            (ids::PROFILE_EMAIL, "headless@example.com"),
        ] {
            let resp = handle_request(
                host,
                req(Op::SetValue {
                    target: target.into(),
                    value: value.into(),
                }),
                None,
            );
            assert!(resp.ok, "{target}: {:?}", resp.error);
        }
    }

    #[test]
    fn live_app_db_requires_token_unless_path_override() {
        assert!(live_database_token_required(false, false));
        assert!(!live_database_token_required(true, false));
        assert!(!live_database_token_required(false, true));
        assert!(!live_database_token_required(true, true));
    }

    #[test]
    fn default_command_is_serve() {
        let cli = Cli::try_parse_from(["bir-headless"]).unwrap();
        assert_eq!(cli.command, None);
        assert!(!cli.wait);
        assert_eq!(cli.command.unwrap_or(Command::Serve), Command::Serve);
    }

    #[test]
    fn parses_serve_status_shutdown() {
        let serve = Cli::try_parse_from(["bir-headless", "serve"]).unwrap();
        assert_eq!(serve.command, Some(Command::Serve));
        assert!(!serve.wait);
        let status = Cli::try_parse_from(["bir-headless", "status"]).unwrap();
        assert_eq!(status.command, Some(Command::Status));
        let shutdown = Cli::try_parse_from(["bir-headless", "shutdown"]).unwrap();
        assert_eq!(shutdown.command, Some(Command::Shutdown));
        assert!(Cli::try_parse_from(["bir-headless", "nope"]).is_err());
    }

    #[test]
    fn parses_wait_on_default_and_serve() {
        let default_wait = Cli::try_parse_from(["bir-headless", "--wait"]).unwrap();
        assert!(default_wait.wait);
        assert_eq!(default_wait.command, None);
        let serve_wait = Cli::try_parse_from(["bir-headless", "serve", "--wait"]).unwrap();
        assert!(serve_wait.wait);
        assert_eq!(serve_wait.command, Some(Command::Serve));
        let wait_before = Cli::try_parse_from(["bir-headless", "--wait", "serve"]).unwrap();
        assert!(wait_before.wait);
        assert_eq!(wait_before.command, Some(Command::Serve));
    }

    #[test]
    fn open_serve_database_does_not_quarantine_on_reopen() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("bir_data.db");
        temp_env::with_vars(
            [
                ("EBIR_TEST_ENV", Some("1")),
                ("BIR_DATABASE_PATH", Some(path.to_str().expect("utf8 path"))),
            ],
            || {
                {
                    let db = Database::open(&path).expect("create file db");
                    db.set_setting("probe", "keep")
                        .expect("write before unclean drop");
                }
                let (db, opened) =
                    open_serve_database().expect("reopen must not quarantine the taxpayer file");
                assert_eq!(opened, path);
                assert_eq!(db.get_setting("probe").expect("read"), Some("keep".into()));
                let corrupt = std::fs::read_dir(directory.path())
                    .expect("dir")
                    .filter_map(|entry| entry.ok())
                    .any(|entry| entry.file_name().to_string_lossy().contains("corrupt"));
                assert!(!corrupt, "headless must not open_or_recreate/quarantine");
                let second = match open_serve_database() {
                    Ok(_) => panic!("exclusive live owner"),
                    Err(message) => message,
                };
                assert!(second.contains("already open"), "{second}");
            },
        );
    }

    #[test]
    fn bind_failure_mentions_two_hosts_and_quit_gui() {
        let addr: SocketAddr = "127.0.0.1:17421".parse().unwrap();
        let error = std::io::Error::from(ErrorKind::AddrInUse);
        let message = bind_failure_message(addr, &error);
        assert!(message.contains("in use"), "{message}");
        assert!(message.contains("painted"), "{message}");
        assert!(message.contains("Two AgentHosts"), "{message}");
        assert!(message.contains("serve --wait"), "{message}");
    }

    #[test]
    fn wait_without_flag_refuses_busy_bind() {
        let holder = TcpListener::bind("127.0.0.1:0").expect("hold port");
        let addr = holder.local_addr().expect("addr");
        let error = wait_until_unblocked(addr, false).expect_err("busy bind");
        assert!(error.contains("in use"), "{error}");
        drop(holder);
    }

    #[test]
    fn wait_until_unblocked_binds_after_port_released() {
        let holder = TcpListener::bind("127.0.0.1:0").expect("hold port");
        let addr = holder.local_addr().expect("addr");
        let (tx, rx) = std::sync::mpsc::channel();
        let waiter = thread::spawn(move || {
            tx.send(wait_until_unblocked(addr, true)).ok();
        });
        thread::sleep(Duration::from_millis(250));
        drop(holder);
        let result = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("waiter finished after bind release");
        assert!(result.is_ok(), "{result:?}");
        waiter.join().expect("waiter thread");
    }

    #[test]
    fn wait_until_unblocked_opens_after_owner_lock_released() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("wait_lock.db");
        let path_s = path.to_str().expect("utf8 path");
        temp_env::with_vars(
            [
                ("EBIR_TEST_ENV", Some("1")),
                ("BIR_DATABASE_PATH", Some(path_s)),
            ],
            || {
                let lock = db::try_acquire_live_owner_lock(&path).expect("hold owner lock");
                let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
                let refused = wait_until_unblocked(addr, false).expect_err("busy lock");
                assert!(refused.contains("already open"), "{refused}");
                let (tx, rx) = std::sync::mpsc::channel();
                let waiter = thread::spawn(move || {
                    tx.send(wait_until_unblocked(addr, true)).ok();
                });
                thread::sleep(Duration::from_millis(250));
                drop(lock);
                let result = rx
                    .recv_timeout(Duration::from_secs(5))
                    .expect("waiter finished after lock release");
                assert!(result.is_ok(), "{result:?}");
                waiter.join().expect("waiter thread");
            },
        );
    }

    #[test]
    fn serve_without_wait_exits_when_bind_busy() {
        let holder = TcpListener::bind("127.0.0.1:0").expect("hold port");
        let addr = holder.local_addr().expect("addr");
        let addr_s = addr.to_string();
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("refuse.db");
        let path_s = path.to_str().expect("utf8 path").to_string();
        temp_env::with_vars(
            [
                ("GPUI_AGENT", Some("1")),
                ("GPUI_AGENT_TOKEN", Some("wait-secret")),
                ("GPUI_AGENT_ADDR", Some(addr_s.as_str())),
                ("BIR_DATABASE_PATH", Some(path_s.as_str())),
                ("EBIR_TEST_ENV", Some("1")),
            ],
            || {
                let error = serve(false).expect_err("refuse busy bind");
                assert_eq!(error, ExitCode::from(2));
            },
        );
        drop(holder);
    }

    #[test]
    fn serve_wait_binds_after_port_and_lock_released() {
        let holder = TcpListener::bind("127.0.0.1:0").expect("hold port");
        let addr = holder.local_addr().expect("addr");
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("wait.db");
        let addr_s = addr.to_string();
        let path_s = path.to_str().expect("utf8 path").to_string();
        let token = "wait-secret";
        temp_env::with_vars(
            [
                ("GPUI_AGENT", Some("1")),
                ("GPUI_AGENT_TOKEN", Some(token)),
                ("GPUI_AGENT_ADDR", Some(addr_s.as_str())),
                ("BIR_DATABASE_PATH", Some(path_s.as_str())),
                ("EBIR_TEST_ENV", Some("1")),
            ],
            || {
                let lock = db::try_acquire_live_owner_lock(&path).expect("hold owner lock");
                let server = thread::spawn(|| serve(true));
                thread::sleep(Duration::from_millis(300));
                let mut blocked =
                    AgentClient::connect(addr).with_timeout(Duration::from_millis(200));
                blocked = blocked.with_token(token.to_string());
                assert!(
                    blocked.wait_ready().is_err(),
                    "headless must not dual-write while bind+lock are held"
                );
                drop(lock);
                drop(holder);
                let started = Instant::now();
                let mut ready = false;
                while started.elapsed() < Duration::from_secs(8) {
                    let mut client =
                        AgentClient::connect(addr).with_timeout(Duration::from_secs(1));
                    client = client.with_token(token.to_string());
                    if client.wait_ready().is_ok() {
                        let hello = client.expect_ok(Op::Hello).expect("hello");
                        assert_eq!(
                            hello.hello.as_ref().map(|info| info.platform),
                            Some(PlatformKind::Headless),
                            "{hello:?}"
                        );
                        client
                            .invoke("profile.list", json!({}))
                            .expect("profile.list after wait");
                        client.expect_ok(Op::Shutdown).expect("shutdown");
                        ready = true;
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                assert!(ready, "serve --wait never acquired bind+DB after release");
                let joined = server.join().expect("serve thread");
                assert!(joined.is_ok(), "{joined:?}");
            },
        );
    }

    #[test]
    fn headless_file_db_profile_save_is_visible_on_reopen() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path: PathBuf = directory.path().join("bir_data.db");
        temp_env::with_var("EBIR_TEST_ENV", Some("1"), || {
            let db = Arc::new(Mutex::new(
                Database::open(&path).expect("file-backed SQLCipher, not ephemeral"),
            ));
            {
                let mut host = BirAgentHost::new(PlatformKind::Headless).with_database(db.clone());
                fill_profile_editor(&mut host, "00000000000002", "Headless Live TIN");
                let saved = handle_request(
                    &mut host,
                    req(Op::Invoke {
                        name: "profile.save".into(),
                        args: json!({}),
                    }),
                    None,
                );
                assert!(saved.ok, "{:?}", saved.error);
                let listed = handle_request(
                    &mut host,
                    req(Op::Invoke {
                        name: "profile.list".into(),
                        args: json!({}),
                    }),
                    None,
                );
                assert!(listed.ok, "{:?}", listed.error);
                assert!(
                    listed
                        .result
                        .as_ref()
                        .unwrap()
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|row| {
                            row["tin"] == "00000000000002" && row["name"] == "Headless Live TIN"
                        }),
                    "{:?}",
                    listed.result
                );
            }
            assert!(path.is_file(), "serve DB must be a real file");
            assert!(
                std::fs::metadata(&path).expect("meta").len() > 0,
                "file-backed db must not be empty"
            );
            db.lock().expect("db").checkpoint().expect("wal checkpoint");
            drop(db);
            let reopened = Database::open(&path).expect("reopen after host drop");
            let listed = reopened.list_profiles().expect("list");
            assert!(
                listed
                    .iter()
                    .any(|profile| profile.tin.full() == "00000000000002"
                        && profile.full_name == "Headless Live TIN"),
                "{listed:?}"
            );
        });
    }

    #[test]
    fn headless_spawn_host_persists_profile_save_to_file_db() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("bir_data.db");
        temp_env::with_var("EBIR_TEST_ENV", Some("1"), || {
            let db = Arc::new(Mutex::new(Database::open(&path).expect("file db")));
            let host = Arc::new(Mutex::new(
                BirAgentHost::new(PlatformKind::Headless).with_database(db.clone()),
            ));
            let (addr, shutdown) =
                spawn_host("127.0.0.1:0".parse().unwrap(), None, host.clone()).expect("bind");
            let mut client = AgentClient::connect(addr).with_timeout(Duration::from_secs(5));
            client.wait_ready().expect("hello");
            client
                .invoke("profile.create", json!({}))
                .expect("create editor");
            for (target, value) in [
                (ids::PROFILE_TIN, "00000000000002"),
                (ids::PROFILE_NAME, "Headless Tcp TIN"),
                (ids::PROFILE_RDO, "018"),
                (ids::PROFILE_LOB, "Retail"),
                (ids::PROFILE_ADDRESS, "Manila"),
                (ids::PROFILE_ZIP, "1000"),
                (ids::PROFILE_PHONE, "09170000000"),
                (ids::PROFILE_EMAIL, "headless-tcp@example.com"),
            ] {
                client
                    .expect_ok(Op::SetValue {
                        target: target.into(),
                        value: value.into(),
                    })
                    .unwrap_or_else(|error| panic!("{target}: {error}"));
            }
            client
                .invoke("profile.save", json!({}))
                .expect("confirm-gated persist is explicit profile.save");
            client.expect_ok(Op::Shutdown).unwrap();
            let started = std::time::Instant::now();
            while !shutdown.load(std::sync::atomic::Ordering::SeqCst)
                || Arc::strong_count(&host) > 1
            {
                assert!(
                    started.elapsed() < Duration::from_secs(2),
                    "headless serve did not drop the host after shutdown"
                );
                thread::sleep(Duration::from_millis(10));
            }
            drop(host);
            db.lock().expect("db").checkpoint().expect("wal checkpoint");
            drop(db);
            let reopened = Database::open(&path).expect("reopen");
            let listed = reopened.list_profiles().expect("list");
            assert!(
                listed
                    .iter()
                    .any(|profile| profile.tin.full() == "00000000000002"),
                "{listed:?}"
            );
        });
    }

    #[test]
    fn second_spawn_host_on_same_addr_is_addr_in_use() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("hold port");
        let addr = listener.local_addr().expect("addr");
        let host = Arc::new(Mutex::new(empty_host()));
        let error = spawn_host(addr, None, host).expect_err("second bind must fail");
        let message = bind_failure_message(addr, &error);
        assert_eq!(error.kind(), ErrorKind::AddrInUse);
        assert!(message.contains("in use"), "{message}");
        drop(listener);
    }
}
