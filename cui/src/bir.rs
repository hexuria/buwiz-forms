//! The door for the CUI app: every host-function call runs on one engine thread.
//!
//! The door's `HostModule` is thread-local (the database slot is too), so — as in the NSH app, where the
//! engine thread ran every call — this crate keeps ONE engine thread and funnels every call through it.
//! The UI thread sends a request and waits for the answer; async host functions are awaited on the engine
//! thread, so the UI stays responsive while they run.
//!
//! Everything the engine owns is reached through the same channel: `desktop::poll`, the print preview's
//! state and the agent command queue live on the engine thread (their mailboxes are thread-local there).

use std::cell::Cell;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use serde_json::Value;

/// Async host calls the UI has not yet collected. CUI only draws when asked, so the frame loop
/// requests redraws while this is non-zero — otherwise a reply that lands while the window is idle
/// would sit unread until the next input event.
static ASYNC_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

/// How many spawned async calls still await collection on the UI thread.
pub fn async_in_flight() -> usize {
    ASYNC_IN_FLIGHT.load(Ordering::SeqCst)
}

/// The receiving end of [`call_async_spawn`]: counts the call as in flight until its answer is
/// collected (or the receiver is dropped), so the frame loop knows to keep drawing.
pub struct AsyncReply {
    rx: Receiver<Result<Value, String>>,
    live: Cell<bool>,
}

impl AsyncReply {
    /// Same contract as `Receiver::try_recv`, plus releasing the in-flight count once the call
    /// resolves or the channel dies.
    pub fn try_recv(&self) -> Result<Result<Value, String>, TryRecvError> {
        let r = self.rx.try_recv();
        match r {
            Ok(_) | Err(TryRecvError::Disconnected) => self.release(),
            Err(TryRecvError::Empty) => {}
        }
        r
    }

    fn release(&self) {
        if self.live.replace(false) {
            ASYNC_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl std::fmt::Debug for AsyncReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncReply").finish_non_exhaustive()
    }
}

impl Drop for AsyncReply {
    fn drop(&mut self) {
        self.release();
    }
}

/// One request to the engine thread.
enum Request {
    /// A synchronous host function: the engine answers before the next request.
    Call {
        name: String,
        args: Vec<Value>,
        reply: Sender<Result<Value, String>>,
    },
    /// An asynchronous host function: the engine awaits it; the UI polls `take_async`.
    CallAsync {
        name: String,
        args: Vec<Value>,
        reply: Sender<Result<Value, String>>,
    },
    /// `desktop::poll()`: what changed since the last poll.
    DesktopPoll {
        reply: Sender<crate::desktop::DesktopPoll>,
    },
    /// The updater driver's word (`desktop::publish_update`).
    PublishUpdate {
        report: crate::desktop::UpdateReport,
        reply: Sender<Result<(), String>>,
    },
    /// `desktop::auto_download()`.
    AutoDownload { reply: Sender<bool> },
    /// `desktop::instance()`.
    Instance {
        reply: Sender<crate::desktop::InstanceStatus>,
    },
    /// `desktop::update_status()`: the updater's state as the page reads it.
    UpdateStatus {
        reply: Sender<bir_app_api::hooks::UpdateStatus>,
    },
    /// The print preview's open keys and documents, for the preview screen.
    PrintState {
        reply: Sender<crate::print::PreviewStates>,
    },
    /// The next agent command for the UI (`agent::pop_command`).
    AgentCommand { reply: Sender<Option<Value>> },
    /// Quit: stop the worker and close the database (`door::shutdown`).
    Shutdown,
}

static ENGINE: OnceLock<Sender<Request>> = OnceLock::new();

/// The engine thread's inbox. Started lazily by the first request — which is also the door's first call,
/// so the boot order (hooks, send guard, database guard, launch) is unchanged.
fn engine() -> &'static Sender<Request> {
    ENGINE.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Request>();
        std::thread::Builder::new()
            .name("ebirforms-engine".to_owned())
            .spawn(move || run(rx))
            .expect("the engine thread starts");
        tx
    })
}

fn run(rx: Receiver<Request>) {
    while let Ok(request) = rx.recv() {
        match request {
            Request::Call { name, args, reply } => {
                let _ = reply.send(crate::door::call(&name, args));
            }
            Request::CallAsync { name, args, reply } => {
                let result = futures::executor::block_on(crate::door::call_async(name, args));
                let _ = reply.send(result);
            }
            Request::DesktopPoll { reply } => {
                let _ = reply.send(crate::desktop::poll());
            }
            Request::PublishUpdate { report, reply } => {
                let _ = reply.send(crate::desktop::publish_update(report));
            }
            Request::AutoDownload { reply } => {
                let _ = reply.send(crate::desktop::auto_download());
            }
            Request::Instance { reply } => {
                let _ = reply.send(crate::desktop::instance());
            }
            Request::UpdateStatus { reply } => {
                let _ = reply.send(crate::desktop::update_status());
            }
            Request::PrintState { reply } => {
                let _ = reply.send(crate::print::states());
            }
            Request::AgentCommand { reply } => {
                let _ = reply.send(crate::agent::pop_command());
            }
            Request::Shutdown => {
                crate::door::shutdown();
                return;
            }
        }
    }
}

fn ask<T>(request: impl FnOnce(Sender<T>) -> Request) -> T {
    let (tx, rx) = mpsc::channel();
    engine()
        .send(request(tx))
        .expect("the engine thread is alive");
    rx.recv().expect("the engine thread answers")
}

// -------------------------------------------------------------------------------------- host functions

/// Call a synchronous host function and wait for its answer.
pub fn call(name: &str, args: Vec<Value>) -> Result<Value, String> {
    ask(|reply| Request::Call {
        name: name.to_owned(),
        args,
        reply,
    })
}

/// Call a synchronous host function with no arguments.
pub fn call0(name: &str) -> Result<Value, String> {
    call(name, Vec::new())
}

/// Call a synchronous host function with one argument.
pub fn call1(name: &str, arg: Value) -> Result<Value, String> {
    call(name, vec![arg])
}

/// Call an asynchronous host function and wait for its answer on the engine thread. The UI thread
/// blocks; use [`call_async_spawn`] when the wait would stall a frame.
pub fn call_async(name: &str, args: Vec<Value>) -> Result<Value, String> {
    ask(|reply| Request::CallAsync {
        name: name.to_owned(),
        args,
        reply,
    })
}

/// Fire an asynchronous host function without waiting: the answer arrives on `reply` whenever the
/// engine finishes. Several may be in flight at once.
pub fn call_async_spawn(name: &str, args: Vec<Value>) -> AsyncReply {
    let (tx, rx) = mpsc::channel();
    engine()
        .send(Request::CallAsync {
            name: name.to_owned(),
            args,
            reply: tx,
        })
        .expect("the engine thread is alive");
    ASYNC_IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
    AsyncReply {
        rx,
        live: Cell::new(true),
    }
}

/// Whether `name` is an async host function (needs `call_async`, not `call`).
pub fn is_async(name: &str) -> bool {
    crate::door::functions()
        .into_iter()
        .any(|(n, kind)| n == name && kind == bir_app_api::host::FunctionKind::Async)
}

/// How sending stands and which database file is named (`door_status` in the TS door).
pub fn door_status() -> DoorStatus {
    DoorStatus {
        sending: crate::door::send_mode().label().to_owned(),
        database_path: crate::door::database_path().display().to_string(),
        test_environment: crate::door::test_environment(),
    }
}

/// What `bir.doorStatus()` reported to TSX.
#[derive(Debug, Clone)]
pub struct DoorStatus {
    /// "dry_run" (nothing leaves this computer), "mock" (the local mock BIR server) or "refused".
    pub sending: String,
    /// The database file this thread has open, or the one the environment names.
    pub database_path: String,
    /// `EBIR_TEST_ENV`: a dev, test or agent run.
    pub test_environment: bool,
}

// ---------------------------------------------------------------------------------- the desktop face

/// What changed since the last poll: a second launch asking for the window, updater requests, menu refreshes.
pub fn desktop_poll() -> crate::desktop::DesktopPoll {
    ask(|reply| Request::DesktopPoll { reply })
}

/// The updater driver's word on what it is doing (`bir.update_status` answers it).
pub fn publish_update(report: crate::desktop::UpdateReport) -> Result<(), String> {
    ask(|reply| Request::PublishUpdate { report, reply })
}

/// "Download updates automatically": the stored setting (on when never set).
pub fn auto_download() -> bool {
    ask(|reply| Request::AutoDownload { reply })
}

/// How the one-copy guard stands in this process.
/// `bir.update_status`: the updater's state as the page reads it.
pub fn desktop_update_status() -> bir_app_api::hooks::UpdateStatus {
    ask(|reply| Request::UpdateStatus { reply })
}

pub fn instance() -> crate::desktop::InstanceStatus {
    ask(|reply| Request::Instance { reply })
}

/// The print previews' state for the preview screens.
pub fn print_states() -> crate::print::PreviewStates {
    ask(|reply| Request::PrintState { reply })
}

/// The next command the agent host queued for the UI, if any.
pub fn agent_command() -> Option<Value> {
    ask(|reply| Request::AgentCommand { reply })
}

/// Quit: stop the submission worker and close the database, then end the engine thread.
pub fn shutdown() {
    let _ = engine().send(Request::Shutdown);
}
