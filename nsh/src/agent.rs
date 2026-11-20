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

// Linked only to switch on NSH's own window control plane (cargo feature `gpui-agent-server`, see Cargo.toml): the
// generated macOS host and runner share this crate, so the feature reaches them through the one dependency graph.
#[cfg(all(feature = "agent", target_os = "macos"))]
use nsh_platform_macos as _;

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
    /// `note`.
    pub fn start() -> AgentStatus {
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
pub use live::{Listener, listen};

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
        APP, Session, UiQueue, drain_gated, resolve_addr, tree_from_debug, try_start_with,
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

    /// The address to bind for the one the environment asked for. Loopback only. Where the window's own control plane
    /// (NSH's, the same `GPUI_AGENT_ADDR`) is in this process, it binds first and this takes another port.
    fn choose(requested: SocketAddr) -> Result<SocketAddr, String> {
        if !is_loopback_addr(requested) {
            return Err(format!(
                "the NSH app's agent listens on loopback only; GPUI_AGENT_ADDR {requested} is not loopback"
            ));
        }
        if cfg!(target_os = "macos") && std::net::TcpListener::bind(requested).is_err() {
            return Ok(SocketAddr::new(requested.ip(), 0));
        }
        Ok(resolve_addr(requested))
    }

    /// Starts the listener when `GPUI_AGENT=1`, on the calling thread's database: the door boots first (the database
    /// guard and the send guard run before the first request is answered). `Ok(None)`: off. `Err`: refused, why.
    pub fn listen() -> Result<Option<Listener>, String> {
        crate::door::call("system_info", Vec::new())?;
        let Some(started) = try_start_with(choose)? else {
            return Ok(None);
        };
        // The record names the executable `bir`: see RECORD_EXECUTABLE.
        drop(started.discovery);
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
