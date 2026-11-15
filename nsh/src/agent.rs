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
//! * **Send safety.** An agent's queue request goes through [`send_draft`], the hook `agent_send_draft` the host
//!   functions use too: the same validate, standing OK and Queue as the UI. The door's send guard has forced a dry
//!   run before the worker started, so a queued return is never a live send (`tests/agent_send.rs`).
//! * **Discovery.** The record names the app `bir-shell` and the executable `bir`, so `bir-mcp` lists the NSH app
//!   beside the old one; the real executable is in `hello`. The window's own control plane (semantic tree, press,
//!   type, screenshot) is NSH's, a second listener: see `nsh/AGENTS.md`, "Agent remote control".
//!
//! Nothing here changes the process environment.

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
        APP, Session, UiQueue, drain as drain_batch, resolve_addr, tree_from_debug, try_start_with,
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
            let quit = drain_batch(&self.mailbox, &self.session, || tree_from_debug(None));
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
