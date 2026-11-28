//! The agent control plane both hosts share: the gpui-agent session (every
//! `invoke` the assistants know), its UI-command queue, the opt-in rules for
//! the listener, and the batch drain. No UI in here.
//!
//! The TCP thread only posts into [`gpui_agent::AgentMailbox`]. The host
//! drains that mailbox on the thread that owns the live database and the
//! window (the gpui shell's timer; the NSH app's engine thread) and gives
//! [`drain`] the semantic tree it has. Screen changes are queued for the
//! script (`bir.agent_take`, through [`pop_command`]); the TCP thread never
//! touches a window. `bir-shell` keeps only its gpui glue (the timer and the
//! tree of its script view); `nsh/src/agent.rs` is the NSH host's.
//!
//! The `bir` command (feature `headless`, crates/bir-cli) runs the same [`Session`] without a
//! window: its UI commands are discarded and screenshots are unavailable.

pub mod request_log;
pub mod session;

#[cfg(feature = "headless")]
pub mod headless;
#[cfg(feature = "headless")]
pub mod headless_daemon;

pub use session::{Session, UiQueue, send_draft};

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_agent::discovery;
use gpui_agent::mailbox::{AgentMailbox, MailboxRequest};
use gpui_agent::protocol::PlatformKind;
use gpui_agent::security::{AgentConfig, SecurityError, authorize_bind, from_env};
use gpui_agent::server::{default_addr, spawn_mailbox_named};
use gpui_agent::tree::{UiNode, UiTree, role};
use gpui_agent::{authorize_request, handle_request, virtual_unavailable};
use serde_json::Value;

/// `app` in this host's discovery record and `hello`.
pub const APP: &str = "bir-shell";

static UI_COMMANDS: Mutex<Option<session::UiQueue>> = Mutex::new(None);

/// Pop the next command for `bir.agent_take`. Null when the listener is off.
pub fn pop_command() -> Option<Value> {
    UI_COMMANDS
        .lock()
        .expect("agent ui queue")
        .as_ref()
        .and_then(session::UiQueue::pop)
}

/// A running listener: what the host's drain needs, and what keeps the
/// discovery record alive.
pub struct Started {
    pub mailbox: AgentMailbox,
    pub shutdown: Arc<AtomicBool>,
    pub commands: session::UiQueue,
    /// The address the listener bound (an ephemeral port is the real one).
    pub addr: std::net::SocketAddr,
}

/// Wait for the record the serve thread of the library publishes for this
/// listener: `spawn_mailbox_named`'s serve loop writes exactly one discovery
/// record when it starts — naming the running binary — into the same
/// `hosts/<pid>-<port>.json` path the app's record uses, at a time it
/// chooses. The app's correct record must be written only after that write
/// has landed, or a late serve-thread write overwrites it and names the
/// binary instead of the app's executable. The serve thread writes once, so
/// the record written after it is the last write. Returns as soon as the
/// record exists, or after two seconds (a record the thread never wrote —
/// a read-only runtime dir — cannot overwrite the app's either).
pub fn wait_for_serve_record(addr: &std::net::SocketAddr) {
    let serve_record = discovery::runtime_root().join("hosts").join(format!(
        "{}-{}.json",
        std::process::id(),
        addr.port()
    ));
    let until = std::time::Instant::now() + Duration::from_secs(2);
    while !serve_record.exists() && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Start the listener when `GPUI_AGENT=1`, on the address the environment
/// names ([`resolve_addr`]). Why it is off or refused is logged.
pub fn maybe_start() -> Option<Started> {
    try_start_with(|requested| Ok(resolve_addr(requested)))
        .ok()
        .flatten()
}

/// [`maybe_start`] with the host's say over the address, and the reason when
/// it does not start: `Ok(None)` is "off" (`GPUI_AGENT=1` is not set), `Err`
/// is a refusal or a failed bind, in a sentence. `choose` gets the address
/// the environment asked for and answers the one to bind, or refuses it. A
/// host that shares the environment's address with another listener (the NSH
/// window's control plane) answers another one; a host that listens on
/// loopback only refuses the rest.
pub fn try_start_with(
    choose: impl FnOnce(std::net::SocketAddr) -> Result<std::net::SocketAddr, String>,
) -> Result<Option<Started>, String> {
    try_start_with_token(None, choose)
}

/// The listener's configuration when the environment opted in without a token: the host's `token` stands in for it,
/// on the address `GPUI_AGENT_ADDR` names (`requested`) or the default. Every other answer of the environment stays.
fn with_fallback_token(
    from_environment: Result<Option<AgentConfig>, SecurityError>,
    requested: Option<String>,
    token: Option<String>,
) -> Result<Option<AgentConfig>, SecurityError> {
    match (from_environment, token.filter(|token| !token.is_empty())) {
        (Err(SecurityError::LoopbackRequiresToken), Some(token)) => requested
            .map_or_else(
                || Ok(default_addr()),
                |raw| raw.parse().map_err(|_| SecurityError::BadAddr(raw)),
            )
            .and_then(|addr| authorize_bind(addr, Some(&token), false))
            .map(|addr| {
                Some(AgentConfig {
                    addr,
                    token: Some(token),
                })
            }),
        (other, _) => other,
    }
}

/// [`try_start_with`] for a host whose launcher keeps `GPUI_AGENT_TOKEN` out of the process environment (NSH's
/// phone hosts get the window plane's token as a private file that the window plane reads and deletes): when the
/// environment opts in (`GPUI_AGENT=1`) but carries no token, `token` is the one this listener checks. A token in
/// the environment wins, and every other rule of the environment (the release gate, the address) still applies.
pub fn try_start_with_token(
    token: Option<String>,
    choose: impl FnOnce(std::net::SocketAddr) -> Result<std::net::SocketAddr, String>,
) -> Result<Option<Started>, String> {
    let from_environment =
        with_fallback_token(from_env(), std::env::var("GPUI_AGENT_ADDR").ok(), token);
    let config = match from_environment {
        Ok(Some(config)) => config,
        Ok(None) => {
            tracing::info!("gpui-agent control plane off (set GPUI_AGENT=1 to opt in)");
            return Ok(None);
        }
        Err(error) => {
            tracing::warn!(%error, "gpui-agent control plane refused");
            return Err(error.to_string());
        }
    };
    try_start_config(config, choose)
}

/// [`try_start_with`] for a host that reads the opt-in itself (Android, whose app process has no launch
/// environment): `config` is the address asked for and the token, already decided.
pub fn try_start_config(
    config: gpui_agent::security::AgentConfig,
    choose: impl FnOnce(std::net::SocketAddr) -> Result<std::net::SocketAddr, String>,
) -> Result<Option<Started>, String> {
    let bind = match choose(config.addr) {
        Ok(bind) => bind,
        Err(error) => {
            tracing::warn!(%error, "gpui-agent control plane refused");
            return Err(error);
        }
    };
    let mailbox = AgentMailbox::new();
    let commands = session::UiQueue::new();
    match spawn_mailbox_named(
        bind,
        config.token,
        mailbox.clone(),
        Duration::from_secs(8),
        APP,
    ) {
        Ok((addr, shutdown)) => {
            tracing::info!(%addr, "gpui-agent listening (platform=desktop, app=bir-shell, protocol v2 HMAC)");
            say(&format!(
                "gpui-agent listening on {addr} (platform=desktop, app={APP})"
            ));
            // The serve thread writes the only record the library ever
            // writes; the app's correct record must come after it. Nothing
            // is published here: a record written now would name this
            // binary, and the executable the record must carry is the
            // host's (`bir`, `bir-headless`), which only the caller knows.
            wait_for_serve_record(&addr);
            install_queue(commands.clone());
            Ok(Some(Started {
                mailbox,
                shutdown,
                commands,
                addr,
            }))
        }
        Err(error) => {
            tracing::error!(%error, "gpui-agent failed to bind");
            Err(format!("cannot bind {bind}: {error}"))
        }
    }
}

/// A line on stderr. A write that fails is ignored: a host whose stderr is a closed pipe (a launcher that reads nothing) must
/// not panic where `eprintln!` would.
pub fn say(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "{line}");
}

/// Answer the whole batch with the sign-in error when the account gate is
/// locked, and do no host work. `Some` is the batch to run.
pub fn admit(batch: Vec<MailboxRequest>) -> Option<Vec<MailboxRequest>> {
    if let Err(error) = crate::account_gate::ensure_unlocked() {
        let message = error.to_string();
        for posted in batch {
            let id = posted.request.id.clone();
            posted.reply(gpui_agent::Response::err(id, message.clone()));
        }
        return None;
    }
    Some(batch)
}

/// Make `queue` the one `bir.agent_take` reads ([`pop_command`]).
pub fn install_queue(queue: session::UiQueue) {
    *UI_COMMANDS.lock().expect("agent ui queue") = Some(queue);
}

/// Keep an explicit `GPUI_AGENT_ADDR`. Otherwise use an ephemeral port when
/// the default is already taken.
pub fn resolve_addr(requested: std::net::SocketAddr) -> std::net::SocketAddr {
    let explicit = std::env::var_os("GPUI_AGENT_ADDR").is_some_and(|value| !value.is_empty());
    if explicit || std::net::TcpListener::bind(requested).is_ok() {
        return requested;
    }
    std::net::SocketAddr::new(requested.ip(), 0)
}

/// Answer every request of `batch` with `message` as the error, and do no host work.
fn refuse(batch: Vec<MailboxRequest>, message: &str) {
    for posted in batch {
        let id = posted.request.id.clone();
        posted.reply(gpui_agent::Response::err(id, message.to_owned()));
    }
}

/// Run one drain of `mailbox` on the host's thread; `true` when the session
/// asked the host to quit. `tree` gives the semantic tree the host has now;
/// it is only asked for when a batch is waiting.
pub fn drain(
    mailbox: &AgentMailbox,
    session: &std::cell::RefCell<Session>,
    tree: impl FnOnce() -> UiTree,
) -> bool {
    drain_gated(mailbox, session, tree, || Ok(()))
}

/// [`drain`] for a host that has a say of its own, after the account gate: `gate` is asked once per waiting batch and an
/// `Err` answers every request of it with that sentence, doing no host work (the NSH app refuses while its sign-in lock is up
/// or a taxpayer is signed in; the old shell has no such state and calls [`drain`]).
pub fn drain_gated(
    mailbox: &AgentMailbox,
    session: &std::cell::RefCell<Session>,
    tree: impl FnOnce() -> UiTree,
    gate: impl FnOnce() -> Result<(), String>,
) -> bool {
    let batch = mailbox.take();
    if batch.is_empty() {
        return false;
    }
    // Every gpui-agent / bir-mcp tool hits this drain. The account gate is
    // checked on each batch, including after a cached database, so signing
    // out locks the tools again. The test bypass is the gate's own: both
    // EBIR_TEST_ENV=1 and BIR_AUTH_BYPASS=1, and only in debug builds.
    let Some(batch) = admit(batch) else {
        return false;
    };
    if let Err(message) = gate() {
        refuse(batch, &message);
        return false;
    }
    // A3a §A.5: the cached handle is compared with the open room's generation on each batch. On a
    // change the session is rebuilt — selected_tin = None, open = None, a new db.
    {
        // The machine's one read (§1): the slot is gone — the generation asked after is the
        // open room's in the snapshot, and there is no other open room.
        let current = crate::session::snapshot()
            .room
            .map(|(_, generation, _, _)| generation);
        let stale = {
            let session = session.borrow();
            session.cached_generation() != current
                && (session.cached_generation().is_some() || current.is_some())
        };
        if stale {
            tracing::info!("agent session rebuilt: the open room changed");
            session.borrow_mut().rebuild();
        }
    }
    if session.borrow().needs_db() {
        // X4: the handle and the generation come from one read of the slot — a switch between
        // two reads would pair one room's Arc with the other's generation, and every later
        // batch would read the old room as "not stale". Single-room mode has no slot (no room
        // switches there): the separate reads are safe.
        let pair = match crate::session::snapshot().room {
            Some((handle, generation, _, _)) => Ok((handle, Some(generation))),
            None => crate::bir_module::database_handle().map(|handle| {
                (
                    handle,
                    crate::session::snapshot()
                        .room
                        .map(|(_, generation, _, _)| generation),
                )
            }),
        };
        match pair {
            Ok((handle, generation)) => session.borrow_mut().set_db(handle, generation),
            Err(error) => {
                let message = error.to_string();
                for posted in batch {
                    let id = posted.request.id.clone();
                    posted.reply(gpui_agent::Response::err(id, message.clone()));
                }
                return false;
            }
        }
    }
    session.borrow_mut().set_tree(tree());
    let mut quit = false;
    for posted in batch {
        if let Err(response) = authorize_request(&posted.request, None, None) {
            posted.reply(*response);
            continue;
        }
        if posted.request.op.is_virtual_input() {
            let id = posted.request.id.clone();
            posted.reply(gpui_agent::Response::err(
                id,
                virtual_unavailable(
                    "bir-shell ships semantic delivery; virtual in-window events are not wired",
                ),
            ));
            continue;
        }
        let response = {
            let mut host = session.borrow_mut();
            handle_request(&mut *host, posted.request.clone(), None, None)
        };
        quit = session.borrow().should_quit();
        posted.reply(response);
    }
    quit
}

/// Quoted ids from a gpui-shell `debug_tree`, under one window node.
///
/// Bounds stay zero: this host does not invent layout, so `in_viewport`
/// stays unavailable.
pub fn tree_from_debug(debug: Option<&str>) -> UiTree {
    let mut children = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(text) = debug {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (kind, rest) = trimmed.split_once(' ').unwrap_or((trimmed, ""));
            let Some(id) = first_quoted(rest).or_else(|| first_quoted(trimmed)) else {
                continue;
            };
            if !seen.insert(id.clone()) {
                continue;
            }
            children.push(node_for(kind, &id));
        }
    }
    UiTree {
        app: APP.into(),
        platform: PlatformKind::Desktop,
        ready: true,
        nodes: vec![UiNode::window("window", "eBIRForms").with_children(children)],
        rev: None,
    }
}

fn first_quoted(text: &str) -> Option<String> {
    let start = text.find('"')? + 1;
    let rest = text.get(start..)?;
    let end = rest.find('"')?;
    let id = &rest[..end];
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn node_for(component: &str, id: &str) -> UiNode {
    match component {
        "Button" | "IconButton" => UiNode::button(id, id),
        "Input" | "TextField" | "OtpInput" => UiNode::textbox(id, id),
        "Checkbox" | "Switch" => UiNode::checkbox(id, id),
        "Dialog" => UiNode::dialog(id, id),
        _ => UiNode::new(id, role::NOTE, id),
    }
}

/// `test` or `real`, from the environment this process started with.
pub fn database_kind() -> &'static str {
    let test_env = bir_core::db::test_environment();
    // An explicit path counts only where single-room mode exists (accounts-and-lock I3).
    let override_path = bir_core::db::single_room_database();
    database_kind_for(
        test_env,
        override_path.as_deref(),
        &bir_core::platform::data_dir(),
    )
}

/// `test` when `EBIR_TEST_ENV` is on (`bir_core::db::test_environment`)
/// or `BIR_DATABASE_PATH` points outside the default data directory; `real`
/// otherwise.
pub fn database_kind_for(
    test_env: bool,
    override_path: Option<&Path>,
    data_dir: &Path,
) -> &'static str {
    if test_env {
        return "test";
    }
    match override_path {
        Some(path) if !canonical(path).starts_with(canonical(data_dir)) => "test",
        _ => "real",
    }
}

fn canonical(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    if let Ok(resolved) = absolute.canonicalize() {
        return resolved;
    }
    match (absolute.parent(), absolute.file_name()) {
        (Some(parent), Some(name)) => canonical(parent).join(name),
        _ => absolute,
    }
}

/// Publish a host's discovery record, retrying when the final rename is
/// refused because another process holds the record file open without delete
/// sharing (Windows: ERROR_SHARING_VIOLATION 32, ERROR_ACCESS_DENIED 5, both
/// surfacing as [`ErrorKind::PermissionDenied`]). A reader that keeps the file
/// open for a moment must not cost the host its record: the rename is tried
/// again up to five times with 50 ms between tries. Every other error fails
/// at once, exactly as `gpui_agent::discovery::publish_record` does.
pub fn publish_record_retrying(
    root: &Path,
    record: &discovery::HostRecord,
) -> std::io::Result<discovery::PublishedHost> {
    const RETRIES: u32 = 5;
    const BETWEEN: Duration = Duration::from_millis(50);
    let mut attempt = 0;
    loop {
        match discovery::publish_record(root, record) {
            Ok(published) => return Ok(published),
            Err(error) if attempt < RETRIES && publish_sharing_error(&error) => {
                attempt += 1;
                std::thread::sleep(BETWEEN);
            }
            Err(error) => return Err(error),
        }
    }
}

/// Whether a publish failure is the sharing class a retry can outlast.
fn publish_sharing_error(error: &std::io::Error) -> bool {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        return true;
    }
    #[cfg(windows)]
    if matches!(error.raw_os_error(), Some(32) | Some(5)) {
        return true;
    }
    let _ = error;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn publish_test_record(port: u16) -> discovery::HostRecord {
        discovery::HostRecord {
            addr: format!("127.0.0.1:{port}"),
            pid: std::process::id(),
            app: "bir-shell".to_owned(),
            executable: "bir".to_owned(),
            bundle_id: None,
            connect_id: None,
        }
    }

    /// Windows: a reader holding the record file open without delete sharing
    /// for ~120 ms must not cost the host its record — the retry outlasts it.
    #[cfg(windows)]
    #[test]
    fn a_sharing_reader_is_outlasted_by_the_publish_retry() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_DELETE: u32 = 0x00000004;
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let record = publish_test_record(48901);
        let first = publish_record_retrying(root, &record).expect("first publish");
        let path = first.path().to_path_buf();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x00000001 | 0x00000002 & !FILE_SHARE_DELETE)
            .open(&path)
            .expect("open without delete sharing");
        let released = std::sync::Arc::new(std::sync::Mutex::new(Some(held)));
        let thread_release = released.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(120));
            thread_release.lock().expect("lock").take();
        });
        let mut rewritten = record.clone();
        rewritten.app = "bir-shell-retried".to_owned();
        let published =
            publish_record_retrying(root, &rewritten).expect("publish retried past the reader");
        let body = std::fs::read_to_string(published.path()).expect("read record");
        assert!(
            body.contains("bir-shell-retried"),
            "the record holds the new publish: {body}"
        );
    }

    /// Windows: a reader holding the file open for two full seconds outlasts
    /// the retries — the publish fails instead of hanging.
    #[cfg(windows)]
    #[test]
    fn a_held_reader_fails_the_publish_after_the_retries() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_DELETE: u32 = 0x00000004;
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let record = publish_test_record(48902);
        let first = publish_record_retrying(root, &record).expect("first publish");
        let path = first.path().to_path_buf();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x00000001 | 0x00000002 & !FILE_SHARE_DELETE)
            .open(&path)
            .expect("open without delete sharing");
        let released = std::sync::Arc::new(std::sync::Mutex::new(Some(held)));
        let thread_release = released.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            thread_release.lock().expect("lock").take();
        });
        let started = std::time::Instant::now();
        let result = publish_record_retrying(root, &record);
        assert!(
            result.is_err(),
            "the publish gives up while the file is held"
        );
        assert!(
            started.elapsed() < Duration::from_millis(1500),
            "no hang: gave up in {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn debug_tree_ids_become_nodes_with_zero_bounds() {
        let tree = tree_from_debug(Some(
            "Button \"nav-home\"\n  Input \"frm1701q:txtSheets\"\nButton \"nav-home\"\nText \"Home\"\n",
        ));
        assert_eq!(tree.app, APP);
        assert_eq!(tree.platform, PlatformKind::Desktop);
        let window = &tree.nodes[0];
        assert_eq!(window.id, "window");
        assert!(!window.bounds.has_area());
        let home = tree.find("nav-home").unwrap();
        assert_eq!(home.role, role::BUTTON);
        assert!(!home.bounds.has_area());
        assert_eq!(tree.find("frm1701q:txtSheets").unwrap().role, role::TEXTBOX);
        assert!(tree.find("Home").is_some());
        assert_eq!(tree.find_all("nav-home").len(), 1);
        assert!(tree_from_debug(None).find("nav-home").is_none());
    }

    #[test]
    fn a_locked_account_refuses_every_posted_tool() {
        use std::time::Duration;

        use gpui_agent::protocol::{Op, Request};

        let mailbox = AgentMailbox::new();
        let hello = mailbox.push(Request::new("hello", Op::Hello));
        let invoke = mailbox.push(Request::new(
            "list",
            Op::Invoke {
                name: "profile.list".into(),
                args: serde_json::json!({}),
            },
        ));
        crate::account_gate::force_locked(|| {
            assert!(admit(mailbox.take()).is_none());
        });
        for response in [hello, invoke] {
            let response = response.recv_timeout(Duration::from_millis(200)).unwrap();
            assert!(!response.ok);
            assert_eq!(
                response.error.as_deref(),
                Some(crate::account_gate::LOCKED_MESSAGE)
            );
        }
    }

    #[test]
    fn a_host_gate_refuses_every_posted_tool_and_does_no_host_work() {
        use std::cell::RefCell;
        use std::time::Duration;

        use gpui_agent::protocol::{Op, Request};

        crate::account_gate::set_unlocked(true);
        let mailbox = AgentMailbox::new();
        let hello = mailbox.push(Request::new("hello", Op::Hello));
        let invoke = mailbox.push(Request::new(
            "list",
            Op::Invoke {
                name: "profile.list".into(),
                args: serde_json::json!({}),
            },
        ));
        let session = RefCell::new(Session::new(session::UiQueue::new()));
        let quit = drain_gated(
            &mailbox,
            &session,
            || panic!("a refused batch is never given the tree"),
            || Err("The app is locked.".to_owned()),
        );
        assert!(!quit);
        for response in [hello, invoke] {
            let response = response.recv_timeout(Duration::from_millis(200)).unwrap();
            assert!(!response.ok);
            assert_eq!(response.error.as_deref(), Some("The app is locked."));
        }
        assert!(session.borrow().needs_db(), "no database was opened for it");
        // Nothing waiting: the gate is not even asked.
        assert!(!drain_gated(
            &mailbox,
            &session,
            || panic!("no tree"),
            || panic!("the gate is asked only when a batch waits"),
        ));
    }

    #[test]
    fn a_host_token_stands_in_only_when_the_environment_opted_in_without_one() {
        let missing = || Err(SecurityError::LoopbackRequiresToken);
        let given = with_fallback_token(
            missing(),
            Some("127.0.0.1:17441".into()),
            Some("secret".into()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(given.addr, "127.0.0.1:17441".parse().unwrap());
        assert_eq!(given.token.as_deref(), Some("secret"));
        let default = with_fallback_token(missing(), None, Some("secret".into()))
            .unwrap()
            .unwrap();
        assert_eq!(default.addr, default_addr());
        // No token, or an empty one, is still the environment's refusal.
        assert!(matches!(
            with_fallback_token(missing(), None, None),
            Err(SecurityError::LoopbackRequiresToken)
        ));
        assert!(matches!(
            with_fallback_token(missing(), None, Some(String::new())),
            Err(SecurityError::LoopbackRequiresToken)
        ));
        // Off stays off, a bad address is refused, and a remote address is not opened by the host's token.
        assert!(
            with_fallback_token(Ok(None), None, Some("secret".into()))
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            with_fallback_token(missing(), Some("nowhere".into()), Some("secret".into())),
            Err(SecurityError::BadAddr(_))
        ));
        assert!(
            with_fallback_token(
                missing(),
                Some("0.0.0.0:17441".into()),
                Some("secret".into())
            )
            .is_err()
        );
        // The environment's own configuration wins over the host's token.
        let own = AgentConfig {
            addr: default_addr(),
            token: Some("env".into()),
        };
        let kept = with_fallback_token(Ok(Some(own)), None, Some("secret".into()))
            .unwrap()
            .unwrap();
        assert_eq!(kept.token.as_deref(), Some("env"));
    }

    #[test]
    fn database_kind_is_test_only_for_test_env_or_a_path_outside_the_data_dir() {
        let data = Path::new("/tmp/ebir-data");
        assert_eq!(database_kind_for(true, None, data), "test");
        assert_eq!(
            database_kind_for(false, Some(Path::new("/tmp/scratch/shell.db")), data),
            "test"
        );
        assert_eq!(
            database_kind_for(false, Some(Path::new("/tmp/ebir-data/bir.db")), data),
            "real"
        );
        assert_eq!(database_kind_for(false, None, data), "real");
    }

    /// `EBIR_TEST_ENV` is read as bir-core reads it: `0`, `false` or empty
    /// is not a test run (any value used to count).
    #[test]
    fn database_kind_reads_the_test_env_as_bir_core_does() {
        for (value, kind) in [
            (Some("0"), "real"),
            (Some("false"), "real"),
            (Some(""), "real"),
            (None, "real"),
            (Some("1"), "test"),
            (Some("true"), "test"),
        ] {
            temp_env::with_vars(
                [("EBIR_TEST_ENV", value), ("BIR_DATABASE_PATH", None)],
                || assert_eq!(database_kind(), kind, "{value:?}"),
            );
        }
    }
}

/// How long a host waits between mailbox drains, given the time since the
/// last request (`None` before the first one): 1 ms while a client is sending
/// requests back to back, `idle` once it has been quiet for 500 ms.
///
/// Only the wait between drains changes: requests are still taken and
/// answered in arrival order on the one thread that owns the database (and,
/// on desktop, the window), so the system model is unaffected.
pub fn drain_interval(
    since_last_request: Option<std::time::Duration>,
    idle: std::time::Duration,
) -> std::time::Duration {
    const BUSY_WINDOW: std::time::Duration = std::time::Duration::from_millis(500);
    match since_last_request {
        Some(elapsed) if elapsed < BUSY_WINDOW => std::time::Duration::from_millis(1),
        _ => idle,
    }
}

#[cfg(test)]
mod drain_interval_tests {
    use super::drain_interval;
    use std::time::Duration;

    const IDLE: Duration = Duration::from_millis(32);

    #[test]
    fn idle_until_first_request() {
        assert_eq!(drain_interval(None, IDLE), IDLE);
    }

    #[test]
    fn fast_while_busy_then_back_to_idle() {
        assert_eq!(
            drain_interval(Some(Duration::ZERO), IDLE),
            Duration::from_millis(1)
        );
        assert_eq!(
            drain_interval(Some(Duration::from_millis(499)), IDLE),
            Duration::from_millis(1)
        );
        assert_eq!(drain_interval(Some(Duration::from_millis(500)), IDLE), IDLE);
        assert_eq!(drain_interval(Some(Duration::from_secs(60)), IDLE), IDLE);
    }
}
