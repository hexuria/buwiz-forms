//! Agent remote control of the NSH app (cargo feature `agent`, off by default).
//!
//! An external agent (`bir-mcp`, the `gpui-agent` CLI) drives the app and its data over the same gpui-agent
//! protocol as the old app: `hello`, then `invoke` of `app.info`, `profile.*`, `form.*`, `dues.list`, `nav.go`. The
//! session (every invoke), the opt-in rules and the batch drain are `bir_app_api::agent`, one copy shared with the
//! gpui shell; this file is the NSH host around it:
//!
//! * **Opt-in, as the old app.** The listener starts only when the app was built with `--features agent` AND
//!   `GPUI_AGENT=1` is set; a release build also needs `GPUI_AGENT_ALLOW_RELEASE=1`; a loopback address needs
//!   `GPUI_AGENT_TOKEN` (an HMAC challenge per connection, the token never crosses the wire) unless
//!   `GPUI_AGENT_INSECURE_NO_TOKEN=1`. The NSH host listens on loopback ONLY (it refuses any other
//!   `GPUI_AGENT_ADDR`, token or not). Without the feature nothing here opens a port.
//! * **The engine thread.** The database slot is per engine thread, so the mailbox is drained there: TSX calls
//!   [`Agent::drain`] every 32 ms (the old app's GPUI timer; `shell/agent`) and the TCP thread only posts requests.
//!   The door boots first, so the database guard and the send guard have run before the first request. (A task of
//!   our own would need an owner: a sync call from TSX has none, so the timer is TSX's.)
//! * **UI commands.** What the session wants shown (`go`, `open`, `refresh`, `choose`, `review`, `quit`) is queued
//!   and read by TSX through the host function `agent_take` ([`pop_command`], the hook of the same name); `nsh/shell/agent`
//!   applies them to the frame. Polled only while [`AgentStatus::active`].
//! * **The app's own locks.** The drain refuses every request, with a sentence, while the app is locked (the Buwiz account gate or
//!   the PIN sign-in screen is in front, or the frame has not booted) and while a taxpayer is signed in. The frame says which it is
//!   through `Agent.setAccess` (an effect of the root, on every change). The default is locked: nothing is answered until the frame
//!   says it is open. First-run setup is not a lock (there is nothing to protect yet, and the agent's `profile.save` may create the
//!   first taxpayer there). The agent's tools read and write the whole database; the UI limits a taxpayer's session to that
//!   taxpayer and this layer cannot limit a request to one, so a taxpayer session refuses data requests entirely. The old shell has
//!   neither state and does not use the gate.
//! * **Send safety.** An agent's queue request goes through [`send_draft`], the hook `agent_send_draft` the host
//!   functions use too: the same validate, standing OK and Queue as the UI. The door's send guard has forced a dry
//!   run before the worker started, so a queued return is never a live send (`tests/agent_send.rs`).
//! * **Discovery.** The record names the app `bir-shell` and the executable `bir`, so `bir-mcp` lists the NSH app
//!   beside the old one; the real executable is in `hello`. The window's own control plane (semantic tree, press,
//!   type, screenshot) is NSH's, a second listener: see `nsh/AGENTS.md`, "Agent remote control".
//!
//! Nothing here changes the process environment.

use std::cell::Cell;

use nsh_core::prelude::*;

// The plug's window control plane (semantic tree, press, type, screenshot): each platform crate exposes an
// `install()` that registers its surface hook, and it must run before the host opens any surface — `native()`
// calls it (see src/lib.rs). Linked under `agent` only, one dependency graph reaching the generated hosts.
#[cfg(all(feature = "agent", target_os = "macos"))]
use nsh_gpui_agent_macos;
#[cfg(all(feature = "agent", target_os = "linux"))]
use nsh_gpui_agent_linux;
#[cfg(all(feature = "agent", target_os = "windows"))]
use nsh_gpui_agent_windows;
#[cfg(all(feature = "agent", target_os = "ios"))]
use nsh_gpui_agent;
// Android links its plug crate for the constructor it self-installs with — no call needed.
#[cfg(all(feature = "agent", target_os = "android"))]
use nsh_gpui_agent_android as _;

/// Installs the plug's window-plane surface hook for this platform's crate. `native()` calls it before the
/// host can open a surface; a no-op without `agent` or on a platform with no plug crate yet.
pub fn install_window_plane() {
    #[cfg(all(feature = "agent", target_os = "macos"))]
    nsh_gpui_agent_macos::install();
    #[cfg(all(feature = "agent", target_os = "linux"))]
    nsh_gpui_agent_linux::install();
    #[cfg(all(feature = "agent", target_os = "windows"))]
    nsh_gpui_agent_windows::install();
    #[cfg(all(feature = "agent", target_os = "ios"))]
    if let Err(error) = nsh_gpui_agent::ios::install() {
        tracing::error!(%error, "the window control plane did not install");
    }
}

/// What the agent remote control is doing.
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct AgentStatus {
    /// The control plane is compiled into this build (cargo feature `agent`).
    pub built: bool,
    /// The listener is up: `GPUI_AGENT=1` was set and its address bound. TSX polls `agent_take` only then.
    pub active: bool,
    /// Where it listens (`127.0.0.1:port`), while active.
    pub address: Option<String>,
    /// A sentence about the state when there is something to say (why it is off or refused).
    pub note: Option<String>,
}

/// The agent remote control: starts the listener the environment asks for and says how it stands.
#[module]
pub struct Agent;

#[module]
impl Agent {
    /// Starts the listener once (a later call answers the status), after the door has booted: when the build has
    /// the `agent` feature and `GPUI_AGENT=1` is set. Call it on the engine thread. Never throws: a refusal is in
    /// `note`. It goes through the door's gate first (`door::enter`), as every call that can boot does: an engine
    /// that quit starts nothing.
    pub fn start(system: System) -> AgentStatus {
        if let Err(refused) = crate::door::enter(&system.locals) {
            return AgentStatus {
                built: cfg!(feature = "agent"),
                active: false,
                address: None,
                note: Some(refused),
            };
        }
        start()
    }

    /// Answers the requests that wait for the agent, on this thread (the one that owns the database). Call it every
    /// 32 ms while `active`. True when the agent asked the app to quit (the `quit` command is queued for TSX).
    pub fn drain() -> bool {
        drain()
    }

    /// How the remote control stands, without starting it.
    pub fn status() -> AgentStatus {
        status()
    }

    /// The frame says who may use the app now: `locked` while the Buwiz gate or the sign-in screen is in front of it (or it has
    /// not booted), `taxpayer_session` while a taxpayer (not the owner or the help desk admin) is signed in. While either is
    /// true the agent's requests are refused: its tools are not limited to one taxpayer's data. Call it on the engine thread,
    /// on every change. Until it is called the app counts as locked.
    pub fn set_access(locked: bool, taxpayer_session: bool) {
        ACCESS.with(|access| access.set(Access { locked, taxpayer_session }));
    }
}

/// Who may use the app, as the frame last said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Access {
    locked: bool,
    taxpayer_session: bool,
}

thread_local! {
    /// The engine thread's access state (`Agent.setAccess`): locked until the frame says otherwise.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; Clippy misreads Android's thread_local expansion"))]
    static ACCESS: Cell<Access> = const { Cell::new(Access { locked: true, taxpayer_session: false }) };
}

/// The sentence of a request refused because the app is locked.
pub const LOCKED: &str = "The app is locked. Sign in on the computer first, then ask again.";

/// The sentence of a request refused because a taxpayer is signed in.
pub const TAXPAYER_SESSION_REFUSED: &str = "A taxpayer is signed in on this computer. The assistant cannot work in a taxpayer's session: sign out, or sign in as the owner or the help desk admin, then ask again.";

/// Why the agent's requests are refused now, or `Ok`: the app is locked, or a taxpayer is signed in. The lock is said first.
pub fn admission(locked: bool, taxpayer_session: bool) -> Result<(), String> {
    if locked {
        Err(LOCKED.to_owned())
    } else if taxpayer_session {
        Err(TAXPAYER_SESSION_REFUSED.to_owned())
    } else {
        Ok(())
    }
}

/// `admission` for what the engine thread knows now.
#[cfg(feature = "agent")]
fn admission_now() -> Result<(), String> {
    let access = ACCESS.with(Cell::get);
    admission(access.locked, access.taxpayer_session)
}

#[cfg(not(feature = "agent"))]
const NOT_BUILT: &str = "This build has no agent control: build the NSH app with --features agent.";

/// The host function `agent_send_draft` hook: run the page's Validate on `draft` and queue it, as the UI does.
#[cfg(feature = "agent")]
pub(crate) fn send_draft(
    db: &bir_core::db::Database,
    kind: bir_core::forms::agent::AgentFormKind,
    draft: &mut serde_json::Value,
) -> Result<serde_json::Value, String> {
    bir_app_api::agent::send_draft(db, kind, draft)
}

/// The next UI command the agent host queued for the script (the host function `agent_take`).
#[cfg(feature = "agent")]
pub(crate) fn pop_command() -> Option<serde_json::Value> {
    bir_app_api::agent::pop_command()
}

#[cfg(not(feature = "agent"))]
pub(crate) fn send_draft(
    _db: &bir_core::db::Database,
    _kind: bir_core::forms::agent::AgentFormKind,
    _draft: &mut serde_json::Value,
) -> Result<serde_json::Value, String> {
    Err(NOT_BUILT.to_owned())
}

#[cfg(not(feature = "agent"))]
pub(crate) fn pop_command() -> Option<serde_json::Value> {
    None
}

#[cfg(not(feature = "agent"))]
fn status() -> AgentStatus {
    AgentStatus {
        built: false,
        active: false,
        address: None,
        note: Some(NOT_BUILT.to_owned()),
    }
}

#[cfg(not(feature = "agent"))]
fn start() -> AgentStatus {
    status()
}

#[cfg(not(feature = "agent"))]
fn drain() -> bool {
    false
}

#[cfg(feature = "agent")]
pub use live::{LaunchToken, Listener, TOKEN_FILE, listen, settings_config, take_token};

#[cfg(feature = "agent")]
fn status() -> AgentStatus {
    live::status()
}

#[cfg(feature = "agent")]
fn start() -> AgentStatus {
    live::start()
}

#[cfg(feature = "agent")]
fn drain() -> bool {
    live::drain()
}

#[cfg(feature = "agent")]
mod live {
    use std::cell::RefCell;
    use std::net::SocketAddr;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use bir_app_api::agent::{
        APP, Session, UiQueue, drain_gated, resolve_addr, tree_from_debug,
    };
    use gpui_agent::discovery::{self, HostRecord, PublishedHost};
    use gpui_agent::mailbox::AgentMailbox;
    use gpui_agent::security::is_loopback_addr;
    use serde_json::json;

    use super::AgentStatus;

    /// The executable the discovery record names, so `bir-mcp` (which lists `bir`, `bir-headless` and `bir-shell`)
    /// finds the NSH app. The real one is in `hello`.
    const RECORD_EXECUTABLE: &str = "bir";

    const OFF: &str =
        "Agent control is off: start the app with GPUI_AGENT=1 and GPUI_AGENT_TOKEN set.";

    /// A listener and its session. Not `Send`: it belongs to the thread that owns the database.
    pub struct Listener {
        mailbox: AgentMailbox,
        session: Rc<RefCell<Session>>,
        commands: UiQueue,
        shutdown: Arc<AtomicBool>,
        addr: SocketAddr,
        _discovery: Option<PublishedHost>,
    }

    impl Listener {
        /// The address it listens on.
        pub fn address(&self) -> SocketAddr {
            self.addr
        }

        /// Answers every request that waits, on this thread; `true` when the session asked the app to quit (`shutdown`),
        /// which also queues the `quit` command for TSX. The semantic tree of this data plane is empty: the window's
        /// is NSH's own control plane.
        pub fn drain(&self) -> bool {
            let quit = drain_gated(
                &self.mailbox,
                &self.session,
                || tree_from_debug(None),
                super::admission_now,
            );
            if quit {
                self.commands.push(json!({"op": "quit"}));
                self.shutdown.store(true, Ordering::SeqCst);
            }
            quit
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::SeqCst);
        }
    }

    /// The address to bind for the one the environment asked for. Loopback only. The window's own control plane
    /// (NSH's development host on every platform, the same `GPUI_AGENT_ADDR`) binds first, and this takes another port.
    fn choose(requested: SocketAddr) -> Result<SocketAddr, String> {
        if !is_loopback_addr(requested) {
            return Err(format!(
                "the NSH app's agent listens on loopback only; GPUI_AGENT_ADDR {requested} is not loopback"
            ));
        }
        // The window's own control plane (the plug, on the same `GPUI_AGENT_ADDR`) binds only once a
        // surface opens — after this listener starts — so it must own the base port and this one moves
        // to the next, as the Android default (`127.0.0.1:17422`) already intends.
        let requested = SocketAddr::new(
            requested.ip(),
            requested.port().checked_add(1).unwrap_or(0),
        );
        if std::net::TcpListener::bind(requested).is_err() {
            return Ok(SocketAddr::new(requested.ip(), 0));
        }
        Ok(resolve_addr(requested))
    }

    /// The opt-in on Android, where the app process has no launch environment: the switch and the address are read from
    /// the development settings (`dev_env`: baked by `just nsh-run-agent android`, or the private `dev.env`), the token
    /// is handed in (never a setting, so never compiled into the app: [`LaunchToken`]). Only a build with those settings
    /// on (`enabled`) can opt in; a token is required. Without `GPUI_AGENT_ADDR` it is the port after NSH's default,
    /// which the window's control plane holds. `var` reads one setting; `token` is asked only once the switch is on.
    pub fn settings_config(
        enabled: bool,
        var: impl Fn(&str) -> Option<String>,
        token: impl FnOnce() -> Option<String>,
    ) -> Result<Option<gpui_agent::security::AgentConfig>, String> {
        let var = |name: &str| var(name).filter(|value| !value.is_empty());
        if !enabled || !matches!(var("GPUI_AGENT").as_deref(), Some("1" | "true")) {
            return Ok(None);
        }
        // The default is the window plane's port: `choose` moves this listener to the port after it (17422).
        let addr = var("GPUI_AGENT_ADDR").unwrap_or_else(|| "127.0.0.1:17421".to_owned());
        let addr: SocketAddr =
            addr.parse().map_err(|_| format!("GPUI_AGENT_ADDR {addr} is not a socket address"))?;
        let token = token()
            .filter(|token| !token.is_empty())
            .ok_or_else(|| format!("GPUI_AGENT=1 needs the token in the private file {TOKEN_FILE}"))?;
        Ok(Some(gpui_agent::security::AgentConfig { addr, token: Some(token) }))
    }

    /// The data plane's token file on Android, in the app's private data directory (beside `dev.env`).
    pub const TOKEN_FILE: &str = "agent-token";

    /// Reads the token from [`TOKEN_FILE`] in `dir` and deletes the file, as NSH's window plane does with its own: the
    /// launcher (`just nsh-run-agent android`) writes it with `adb shell run-as` once the host is installed, which on
    /// a clean device is while the app starts, so a missing file is waited for up to `wait`. The bytes are the token
    /// (compared byte for byte, nothing trimmed). `None`: no file in time, unreadable, empty or over 4 KiB.
    pub fn take_token(dir: &std::path::Path, wait: std::time::Duration) -> Option<String> {
        use std::io::Read;
        let path = dir.join(TOKEN_FILE);
        let until = std::time::Instant::now() + wait;
        loop {
            match std::fs::File::open(&path) {
                Ok(file) => {
                    let mut token = String::new();
                    let read = file.take(4097).read_to_string(&mut token);
                    // Not the launchers': both write `agent-token.new` and rename it into place, so theirs is never
                    // seen half-written. An empty file is some other writer (a hand-made `cat >`) between creating it
                    // and writing it, and is waited for like a missing one.
                    if matches!(read, Ok(0)) && std::time::Instant::now() < until {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        continue;
                    }
                    if let Err(error) = std::fs::remove_file(&path) {
                        tracing::warn!(%error, "agent token file not deleted");
                    }
                    return read.ok().filter(|_| !token.is_empty() && token.len() <= 4096).map(|_| token);
                }
                Err(_) if std::time::Instant::now() < until => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(_) => return None,
            }
        }
    }

    /// The token of one authorized development launch, kept in the process and outside the engine thread, which a full
    /// reload replaces: the launcher writes the file once and [`take_token`] deletes it, so the replacement listener is
    /// authorized with the token this launch already read. A process holds one, so it ends with the launch; a token
    /// file that arrives later (the launcher ran again against the live process) replaces it.
    #[derive(Debug, Default)]
    pub struct LaunchToken(std::sync::Mutex<Option<String>>);

    impl LaunchToken {
        /// A launch that has read no token yet.
        pub const fn new() -> Self {
            Self(std::sync::Mutex::new(None))
        }

        /// The token: the one in [`TOKEN_FILE`] in `dir`, read and deleted as [`take_token`] does and waited for up to
        /// `wait` only while the launch holds none; otherwise the one held.
        pub fn take(&self, dir: &std::path::Path, wait: std::time::Duration) -> Option<String> {
            let mut held = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let wait = if held.is_some() { std::time::Duration::ZERO } else { wait };
            if let Some(token) = take_token(dir, wait) {
                *held = Some(token);
            }
            held.clone()
        }
    }

    /// This process's launch.
    #[cfg(target_os = "android")]
    static LAUNCH: LaunchToken = LaunchToken::new();

    /// The data plane's token on a phone host. NSH hands the window plane's token to its iOS and Android hosts as a
    /// private file that the window plane reads and deletes before the app starts, and keeps `GPUI_AGENT_TOKEN` out of
    /// the app's environment, so this listener would be refused for want of one. `just nsh-run-agent` passes the same
    /// token as `BIR_AGENT_TOKEN` there. Read only where the environment has no `GPUI_AGENT_TOKEN`; never on a desktop.
    /// Android reads its opt-in from the development settings and its token from a private file instead
    /// ([`settings_config`], [`LaunchToken`]).
    #[cfg(not(target_os = "android"))]
    fn phone_token() -> Option<String> {
        if cfg!(target_os = "ios") {
            std::env::var("BIR_AGENT_TOKEN").ok()
        } else {
            None
        }
    }

    /// Starts the listener when `GPUI_AGENT=1`, on the calling thread's database: the door boots first (the database
    /// guard and the send guard run before the first request is answered). `Ok(None)`: off. `Err`: refused, why.
    pub fn listen() -> Result<Option<Listener>, String> {
        crate::door::call("system_info", Vec::new())?;
        #[cfg(not(target_os = "android"))]
        let started = bir_app_api::agent::try_start_with_token(phone_token(), choose)?;
        #[cfg(target_os = "android")]
        let started = match settings_config(
            crate::dev_env::ENABLED,
            |name| bir_core::config::var(name).ok(),
            || LAUNCH.take(&bir_core::platform::data_dir(), std::time::Duration::from_secs(10)),
        )? {
            Some(config) => bir_app_api::agent::try_start_config(config, choose)?,
            None => None,
        };
        let Some(started) = started else {
            return Ok(None);
        };
        // The record names the executable `bir`: see RECORD_EXECUTABLE.
        // `try_start` has already waited out the serve thread's one write,
        // so this correct record is the last write the hosts folder sees.
        let identity = discovery::detect_identity();
        let record = HostRecord {
            addr: started.addr.to_string(),
            pid: std::process::id(),
            app: APP.to_owned(),
            executable: RECORD_EXECUTABLE.to_owned(),
            bundle_id: identity.bundle_id,
            connect_id: identity.connect_id,
        };
        let published = match discovery::publish_record(&discovery::runtime_root(), &record) {
            Ok(published) => Some(published),
            Err(error) => {
                tracing::warn!(%error, "gpui-agent discovery record not written");
                None
            }
        };
        Ok(Some(Listener {
            mailbox: started.mailbox,
            session: Rc::new(RefCell::new(Session::new(started.commands.clone()))),
            commands: started.commands,
            shutdown: started.shutdown,
            addr: started.addr,
            _discovery: published,
        }))
    }

    thread_local! {
        /// The listener of the engine thread, while the remote control runs.
        static RUNNING: RefCell<Option<Listener>> = const { RefCell::new(None) };
        /// Why it is not running, once `start` has answered.
        static NOTE: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    pub(super) fn status() -> AgentStatus {
        let running = RUNNING.with(|slot| slot.borrow().as_ref().map(Listener::address));
        match running {
            Some(addr) => AgentStatus {
                built: true,
                active: true,
                address: Some(addr.to_string()),
                note: None,
            },
            None => AgentStatus {
                built: true,
                active: false,
                address: None,
                note: Some(
                    NOTE.with(|note| note.borrow().clone())
                        .unwrap_or_else(|| OFF.to_owned()),
                ),
            },
        }
    }

    pub(super) fn start() -> AgentStatus {
        if RUNNING.with(|slot| slot.borrow().is_some()) {
            return status();
        }
        let note = match listen() {
            Ok(Some(listener)) => {
                RUNNING.with(|slot| *slot.borrow_mut() = Some(listener));
                None
            }
            Ok(None) => Some(OFF.to_owned()),
            Err(error) => {
                bir_app_api::agent::say(&format!("agent control refused: {error}"));
                Some(format!("Agent control refused: {error}"))
            }
        };
        NOTE.with(|slot| *slot.borrow_mut() = note);
        status()
    }

    pub(super) fn drain() -> bool {
        RUNNING.with(|slot| slot.borrow().as_ref().is_some_and(Listener::drain))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_taken_address_gives_the_next_loopback_port_and_a_remote_one_is_refused() {
            let held = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
            let taken = held.local_addr().expect("its address");
            let chosen = choose(taken).expect("loopback is accepted");
            assert_eq!(chosen, SocketAddr::new(taken.ip(), taken.port() + 1), "the data plane moves to the port after the window plane's");
            assert!(choose("192.0.2.1:17421".parse().expect("an address")).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_refuses_first_then_a_taxpayer_session_and_an_open_frame_is_admitted() {
        assert_eq!(admission(true, false), Err(LOCKED.to_owned()));
        assert_eq!(admission(true, true), Err(LOCKED.to_owned()), "the lock is said first");
        assert_eq!(admission(false, true), Err(TAXPAYER_SESSION_REFUSED.to_owned()));
        assert_eq!(admission(false, false), Ok(()));
    }

    #[test]
    fn the_app_counts_as_locked_until_the_frame_says_otherwise() {
        let now = || ACCESS.with(Cell::get);
        assert_eq!(now(), Access { locked: true, taxpayer_session: false }, "locked at the start");
        Agent::set_access(false, true);
        assert_eq!(now(), Access { locked: false, taxpayer_session: true });
        Agent::set_access(false, false);
        assert_eq!(now(), Access { locked: false, taxpayer_session: false });
        Agent::set_access(true, false);
        assert_eq!(now(), Access { locked: true, taxpayer_session: false });
    }
}
