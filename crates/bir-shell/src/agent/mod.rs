//! Optional gpui-agent control plane for the shell window: the gpui glue.
//!
//! The session, the UI-command queue, the opt-in rules and the batch drain
//! live in `bir_app_api::agent`, shared with the NSH app; this file only
//! drains the mailbox on a GPUI timer (the UI thread owns the live database
//! and the window) and builds the semantic tree from the script view. The TCP
//! thread never touches GPUI. Screen changes are queued for the script
//! (`bir.agent_take`).

pub(crate) use bir_app_api::agent::headless;
pub(crate) use bir_app_api::agent::{pop_command, send_draft};

use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use bir_app_api::agent::{Session, maybe_start, tree_from_debug};
use gpui_agent::mailbox::AgentMailbox;
use gpui_agent::tree::UiTree;
use gpui_kit::{App, Entity, Window};
use gpui_shell::{ScriptView, ShellRoot};

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

fn drain(
    mailbox: &AgentMailbox,
    session: &std::cell::RefCell<Session>,
    root: &Entity<ShellRoot>,
    cx: &mut App,
) -> bool {
    bir_app_api::agent::drain(mailbox, session, || capture_tree(root, cx))
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
