//! Optional gpui-agent control plane for the shell window.
//!
//! The TCP thread only posts into [`gpui_agent::AgentMailbox`]. A GPUI timer
//! drains that mailbox on the UI thread, which is the thread that owns the
//! live database and the window. Screen changes are queued for the script
//! (`bir.agent_take`); the TCP thread never touches GPUI.

mod session;

pub(crate) use session::{Session, UiQueue, load_profile, profile_context, send_draft};

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_agent::discovery::{self, PublishedHost};
use gpui_agent::mailbox::{AgentMailbox, MailboxRequest};
use gpui_agent::protocol::PlatformKind;
use gpui_agent::security::from_env;
use gpui_agent::server::spawn_mailbox_named;
use gpui_agent::tree::{UiNode, UiTree, role};
use gpui_agent::{authorize_request, handle_request, virtual_unavailable};
use gpui_kit::{App, Entity, Window};
use gpui_shell::{ScriptView, ShellRoot};
use serde_json::Value;

/// `app` in this host's discovery record and `hello`.
pub const APP: &str = "bir-shell";

static UI_COMMANDS: Mutex<Option<session::UiQueue>> = Mutex::new(None);

/// Pop the next command for `bir.agent_take`. Null when the listener is off.
pub(crate) fn pop_command() -> Option<Value> {
    UI_COMMANDS
        .lock()
        .expect("agent ui queue")
        .as_ref()
        .and_then(session::UiQueue::pop)
}

/// Start the listener when `GPUI_AGENT=1`, and drain it on `window`.
pub(crate) fn start(window: &mut Window, cx: &mut App, root: Entity<ShellRoot>) {
    let Some(started) = maybe_start() else {
        return;
    };
    let mailbox = started.mailbox;
    let shutdown = started.shutdown;
    let discovery = started.discovery;
    let session = Rc::new(std::cell::RefCell::new(Session::new(started.commands)));
    window
        .spawn(cx, async move |cx| {
            let _discovery = discovery;
            let root = root;
            loop {
                let step = cx.update(|_window, app| {
                    let quit = drain(&mailbox, &session, &root, app);
                    if quit {
                        shutdown.store(true, Ordering::SeqCst);
                        app.quit();
                    }
                    quit
                });
                if !matches!(step, Ok(false)) {
                    shutdown.store(true, Ordering::SeqCst);
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(32))
                    .await;
            }
        })
        .detach();
}

struct Started {
    mailbox: AgentMailbox,
    shutdown: Arc<AtomicBool>,
    discovery: Option<PublishedHost>,
    commands: session::UiQueue,
}

fn maybe_start() -> Option<Started> {
    let config = match from_env() {
        Ok(Some(config)) => config,
        Ok(None) => {
            tracing::info!("gpui-agent control plane off (set GPUI_AGENT=1 to opt in)");
            return None;
        }
        Err(error) => {
            tracing::warn!(%error, "gpui-agent control plane refused");
            return None;
        }
    };
    let mailbox = AgentMailbox::new();
    let commands = session::UiQueue::new();
    match spawn_mailbox_named(
        resolve_addr(config.addr),
        config.token,
        mailbox.clone(),
        Duration::from_secs(8),
        APP,
    ) {
        Ok((addr, shutdown)) => {
            tracing::info!(%addr, "gpui-agent listening (platform=desktop, app=bir-shell, protocol v2 HMAC)");
            eprintln!("gpui-agent listening on {addr} (platform=desktop, app={APP})");
            let discovery = match discovery::publish_current(addr, APP) {
                Ok(published) => Some(published),
                Err(error) => {
                    tracing::warn!(%error, "gpui-agent discovery record not written");
                    None
                }
            };
            install_queue(commands.clone());
            Some(Started {
                mailbox,
                shutdown,
                discovery,
                commands,
            })
        }
        Err(error) => {
            tracing::error!(%error, "gpui-agent failed to bind");
            None
        }
    }
}

/// Answer the whole batch with the sign-in error when the account gate is
/// locked, and do no host work. `Some` is the batch to run.
fn admit(batch: Vec<MailboxRequest>) -> Option<Vec<MailboxRequest>> {
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

fn install_queue(queue: session::UiQueue) {
    *UI_COMMANDS.lock().expect("agent ui queue") = Some(queue);
}

/// Keep an explicit `GPUI_AGENT_ADDR`. Otherwise use an ephemeral port when
/// the default is already taken.
fn resolve_addr(requested: std::net::SocketAddr) -> std::net::SocketAddr {
    let explicit = std::env::var_os("GPUI_AGENT_ADDR").is_some_and(|value| !value.is_empty());
    if explicit || std::net::TcpListener::bind(requested).is_ok() {
        return requested;
    }
    std::net::SocketAddr::new(requested.ip(), 0)
}

fn drain(
    mailbox: &AgentMailbox,
    session: &std::cell::RefCell<Session>,
    root: &Entity<ShellRoot>,
    cx: &mut App,
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
    if session.borrow().needs_db() {
        match crate::bir_module::database_handle() {
            Ok(handle) => session.borrow_mut().set_db(handle),
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
    let tree = capture_tree(root, cx);
    session.borrow_mut().set_tree(tree);
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

fn capture_tree(root: &Entity<ShellRoot>, cx: &App) -> UiTree {
    let script = root
        .read(cx)
        .content()
        .clone()
        .downcast::<ScriptView>()
        .ok();
    let debug = script.and_then(|view| view.read(cx).snapshot().map(|snap| snap.debug_tree()));
    tree_from_debug(debug.as_deref())
}

/// Quoted ids from a gpui-shell `debug_tree`, under one window node.
///
/// Bounds stay zero: this host does not invent layout, so `in_viewport`
/// stays unavailable.
pub(crate) fn tree_from_debug(debug: Option<&str>) -> UiTree {
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
pub(crate) fn database_kind() -> &'static str {
    let test_env = std::env::var_os("EBIR_TEST_ENV").is_some();
    let override_path = std::env::var_os("BIR_DATABASE_PATH")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    database_kind_for(
        test_env,
        override_path.as_deref(),
        &bir_core::platform::data_dir(),
    )
}

/// `test` when `EBIR_TEST_ENV` is set or `BIR_DATABASE_PATH` points outside
/// the default data directory; `real` otherwise.
pub(crate) fn database_kind_for(
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
