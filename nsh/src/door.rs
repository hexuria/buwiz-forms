//! The door: the Rust brain's host functions (`bir_app_api::module()`, the module named `bir`) behind the
//! NSH native module. This file is the plain-Rust half (boot, guards, dispatch); `lib.rs` is its TSX face.
//!
//! * One `HostModule` per engine thread. It holds `Rc` and the database slot is thread-local, so every call
//!   runs on the engine thread, as the old shell's calls ran on its main thread.
//! * The first call on a thread boots it, in the old host's order: hooks, send guard, database guard, the
//!   account gate (`cloud_account::prepare_launch`), then the database and the submission worker.
//! * The guards run before anything opens: development, test, agent and mobile launches do not file live,
//!   and the database guard still protects the owner's data folders.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, Once, OnceLock};

use bir_app_api::host::{FunctionKind, HostArguments, HostModule};
use nsh_core::JsonValue;

use crate::guards::{self, SendMode};
use crate::hooks::NshHooks;

/// What is decided once for the whole process.
struct Process {
    send: SendMode,
}

static PROCESS: OnceLock<Process> = OnceLock::new();

/// Set up the process, once: the keychain switch, the hooks and the send guard.
///
/// Desktop registration sets the keychain and send-guard environment switches before NSH starts
/// threads. Android instead installs in-process configuration overrides at its first Door call,
/// when the injected private data directory is available, before database or worker startup.
/// Neither Android development settings nor its send guard mutate the process environment.
/// Test databases are named with `bir_core::db::set_database_path_override` on every platform.
fn process() -> &'static Process {
    PROCESS.get_or_init(|| set_up(guards::send_guard))
}

#[cfg(target_os = "android")]
pub(crate) fn prepare_android(path: Option<PathBuf>) -> Result<(), String> {
    let path = path.ok_or("The Android engine has no private data directory.")?;
    bir_core::platform::set_data_dir(path.clone()).map_err(|error| error.to_string())?;
    if crate::dev_env::ENABLED {
        static DEV_ENV: OnceLock<Result<(), String>> = OnceLock::new();
        DEV_ENV
            .get_or_init(|| crate::dev_env::load(&path.join("dev.env")))
            .clone()?;
    }
    install_logging();
    Ok(())
}

pub(crate) fn install_logging() {
    static LOGGING: Once = Once::new();
    LOGGING.call_once(|| {
        if test_environment() {
            bir_core::log_files::set_default_log_dir(bir_core::log_files::fresh_test_log_dir());
        }
        let developer_mode = bir_core::logging::developer_mode_enabled();
        let default_filter = if developer_mode {
            "nsh_app_ebirforms=debug,bir_app_api=debug,bir_core=info,bir_submit::ios_storage=info,bir_print=debug,panic=error"
        } else {
            "nsh_app_ebirforms=info,bir_app_api=info,bir_core=info,bir_submit::ios_storage=info,bir_print=error,panic=error"
        };
        if bir_core::logging::install(bir_core::logging::InstallOptions {
            developer_mode,
            default_filter,
        }) {
            tracing::info!("eBIRForms started");
        }
        // The launch storage step ran at registration, before there was a log: its outcome goes out now.
        bir_core::platform::log_app_storage();
    });
}

/// The set-up [`process`] runs, with the send guard given (production passes `guards::send_guard`; a test passes one
/// that refuses, which the environment alone cannot make). A refused send guard holds where the submission worker
/// starts (bir-app-api's latch), so no route can start it, whatever host function it comes through. The name check in
/// [`permitted`] stays, for the clear message.
fn set_up(send_guard: impl FnOnce() -> SendMode) -> Process {
    #[cfg(not(target_os = "android"))]
    switch_off_keychain(cfg!(debug_assertions));
    // iOS: the data folder out of backups and protected, and the old build's Keychain items removed once, after the
    // keychain switch and before the database is opened. Nothing elsewhere. Its outcome is logged once logging starts.
    bir_core::platform::prepare_app_storage();
    let _ = bir_app_api::install_hooks(NshHooks);
    // Phones: the browser for the sign-in, and the phone kept awake while a send is in flight. Desktops register
    // nothing: bir-core opens the browser itself there.
    #[cfg(target_os = "ios")]
    crate::phone_ios::install();
    #[cfg(target_os = "android")]
    crate::phone_android::register();
    let send = send_guard();
    if !send.worker_may_start() {
        bir_app_api::bir_module::refuse_submission_worker();
    }
    Process { send }
}

/// Set up the process now. The native module's registration calls it (see [`process`]).
#[cfg(not(target_os = "android"))]
pub(crate) fn prepare_process() {
    process();
}

/// Whether [`process`] has run in this process.
#[cfg(test)]
pub(crate) fn process_prepared() -> bool {
    PROCESS.get().is_some()
}

/// Debug builds never reach the OS keychain, whatever the caller set: `BUWIZ_NO_KEYCHAIN=1` before anything
/// in bir-core runs. So does any build running in a test environment (`EBIR_TEST_ENV`, read as bir-core reads
/// it): a release build under a test must not reach the keychain either. A release build outside a test
/// environment leaves the variable to the caller.
#[cfg(not(target_os = "android"))]
fn switch_off_keychain(debug_build: bool) {
    if debug_build || test_environment() {
        // SAFETY: called from [`process`], once, before NSH or this crate starts a thread (see there).
        unsafe { std::env::set_var(bir_core::platform::NO_KEYCHAIN_ENV, "1") };
    }
}

/// `EBIR_TEST_ENV` is on: a dev, test or agent run. A database nobody named is a scratch one. Read the way
/// bir-core reads it (`bir_core::db::test_environment`), so the two never disagree.
pub fn test_environment() -> bool {
    bir_core::db::test_environment()
}

const SHELL_POLL: &str = "BIR_SHELL_POLL";

/// Whether a test environment asks for the frame's polls (`BIR_SHELL_POLL=1`): outside one they always run, and
/// this is not what decides it.
pub fn shell_poll() -> bool {
    test_environment() && std::env::var(SHELL_POLL).is_ok_and(|value| value == "1")
}

const DATABASE_PATH: &str = "BIR_DATABASE_PATH";

/// The scratch folders of earlier runs are cleared out the first time this process names one.
static SWEPT: Once = Once::new();
static SCRATCH_SEQUENCE: AtomicU32 = AtomicU32::new(0);
/// One boot at a time: a boot names the database right before this thread opens it.
static BOOT: Mutex<()> = Mutex::new(());

thread_local! {
    /// The database this thread asked for with [`use_database`], until it boots.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; Clippy misreads Android's thread_local expansion"))]
    static NAMED: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Decide which database this thread opens, and name it for bir-core.
///
/// A caller who set `BIR_DATABASE_PATH` has named it; so has a test that called [`use_database`]. In a test
/// environment with neither, a fresh scratch database in the temp folder: a test never opens the NSH app's
/// real data folder, and two engines of one test run never share a file (the owner lock would refuse the
/// second). Nothing is named outside a test environment.
fn choose_database() -> Result<(), String> {
    if !test_environment() {
        return Ok(());
    }
    if let Some(named) = NAMED.with(|slot| slot.borrow().clone()) {
        bir_core::db::set_database_path_override(Some(named));
        return Ok(());
    }
    if !scratch_database_wanted() {
        return Ok(());
    }
    #[cfg(not(target_os = "android"))]
    let root = std::env::temp_dir().join("ebirforms-door");
    #[cfg(target_os = "android")]
    let root = bir_core::platform::temp_dir().join("ebirforms-door");
    SWEPT.call_once(|| forget_old_scratch(&root, SCRATCH_LIFETIME));
    let folder = fresh_scratch_folder(&root, std::process::id(), SCRATCH_SEQUENCE.fetch_add(1, Ordering::SeqCst))
        .map_err(|error| format!("Cannot make a scratch database folder: {error}"))?;
    bir_core::db::set_database_path_override(Some(folder.join("bir_data.db")));
    Ok(())
}

/// An empty folder for the scratch database of this process's engine number `sequence`: `<root>/<pid>-<sequence>`.
///
/// The system gives a process id out again once its process is gone (Windows does so within minutes), and scratch
/// folders are kept for a day. So a folder of this name can already be there, with the database of a run that is
/// over: one a test left a PIN in opened the next test's app at the sign-in screen. No live process but this one
/// has this id, so what is there is nobody's: it goes first.
fn fresh_scratch_folder(root: &Path, pid: u32, sequence: u32) -> std::io::Result<PathBuf> {
    let folder = root.join(format!("{pid}-{sequence}"));
    match std::fs::remove_dir_all(&folder) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    std::fs::create_dir_all(&folder)?;
    Ok(folder)
}

/// Whether the door makes a scratch database of its own (per engine thread, in [`choose_database`]): a test environment
/// whose caller named none (`BIR_DATABASE_PATH` unset or empty). A test's [`use_database`] names one later, per thread.
/// The one-copy guard asks this at registration, before any thread has named anything.
pub(crate) fn scratch_database_wanted() -> bool {
    // The configuration store (Android's dev list) first, then the launch environment.
    test_environment() && !bir_core::config::var(DATABASE_PATH).is_ok_and(|value| !value.is_empty())
}

/// How long a scratch folder lives after its last change.
const SCRATCH_LIFETIME: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Scratch folders of earlier runs that nobody changed for `lifetime` go; best effort, and only ever
/// folders inside `root`.
///
/// A folder's own modified time does not move while a database inside it is written (only adding or
/// removing a file does), so age is the newest change of anything inside, and a folder whose owner lock is
/// held (a live run has its database open, however quiet) is never touched.
fn forget_old_scratch(root: &Path, lifetime: std::time::Duration) {
    forget_old_scratch_with(root, lifetime, |folder| {
        let _ = std::fs::remove_dir_all(folder);
    });
}

/// [`forget_old_scratch`] with the removal of a folder given, so a test can look at what it is doing while the
/// folder goes.
fn forget_old_scratch_with(root: &Path, lifetime: std::time::Duration, remove: impl Fn(&Path)) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let folder = entry.path();
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let quiet = newest_change(&folder)
            .and_then(|changed| changed.elapsed().ok())
            .is_some_and(|age| age > lifetime);
        if !quiet {
            continue;
        }
        // Hold the owner lock until the folder is gone: a run that opens the database now is refused while
        // we delete, and one that already has it open keeps the lock, so we skip. (The lock file is made if
        // the folder has none.)
        let Ok(lock) = bir_core::db::try_acquire_live_owner_lock(&folder.join("bir_data.db"))
        else {
            continue;
        };
        remove(&folder);
        drop(lock);
        // Windows cannot delete the lock file while it is open, so the folder kept it: let it go too. A run that
        // took the lock in between holds that file open (unshared), and Windows refuses to delete an open file,
        // so its folder stays.
        #[cfg(windows)]
        let _ = std::fs::remove_dir_all(&folder);
    }
}

/// The most recent modified time of `path` and of everything inside it. `None` when something cannot be
/// read, which is as good as "changed just now".
fn newest_change(path: &Path) -> Option<std::time::SystemTime> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    let mut newest = meta.modified().ok()?;
    if meta.is_dir() {
        for entry in std::fs::read_dir(path).ok()? {
            newest = newest.max(newest_change(&entry.ok()?.path())?);
        }
    }
    Some(newest)
}

/// Name the database this thread opens (`EBIR_TEST_ENV` only, and before the first call). Returns the path.
/// The database guard applies.
pub fn use_database(path: &str) -> Result<String, String> {
    if !test_environment() {
        return Err("useDatabase is for test environments (EBIR_TEST_ENV=1).".into());
    }
    if BOOTED.with(|slot| slot.borrow().is_some()) {
        return Err(
            "The database is already open on this thread; call useDatabase before anything else."
                .into(),
        );
    }
    let path = PathBuf::from(path);
    if path.as_os_str().is_empty() {
        return Err("useDatabase needs a path.".into());
    }
    if let Some(why) = guards::database_refusal_here(&path) {
        return Err(why);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot make the database folder: {error}"))?;
    }
    let _serial = BOOT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    NAMED.with(|slot| *slot.borrow_mut() = Some(path.clone()));
    bir_core::db::set_database_path_override(Some(path.clone()));
    Ok(path.display().to_string())
}

struct Booted {
    module: Rc<HostModule>,
    /// The database path the guard last passed.
    checked: PathBuf,
    /// The engine that booted the module (its [`Engine`] state). Only that engine is handed it ([`enter`]), and once it
    /// quit no call is answered on it, a direct one included ([`booted`]).
    owner: Rc<Engine>,
}

thread_local! {
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; Clippy misreads Android's thread_local expansion"))]
    static BOOTED: RefCell<Option<Booted>> = const { RefCell::new(None) };
    /// Dropped when the engine thread ends. Touched after the database slot exists, so it is destroyed
    /// before the slot is (thread locals go in reverse order of their first use).
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; Clippy misreads Android's thread_local expansion"))]
    static ARMED: ShutdownGuard = const { ShutdownGuard };
}

/// Stops this engine's submission worker and closes its database when the engine thread goes (the quit path of the
/// old host: `shutdown_for_quit`), on every engine's end: an Android process outlives a Back-exit or a swipe from Recents,
/// and the next engine of the same process starts a worker of its own and opens the database again. `Door.shutdown`
/// does the same on request.
struct ShutdownGuard;

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        // The thread's locals are going (the logger's among them), so the shutdown's work runs on a thread of its own.
        let _ = std::panic::catch_unwind(|| {
            bir_app_api::bir_module::shutdown_at_engine_end();
            let _ = BOOTED.try_with(|slot| slot.borrow_mut().take());
        });
    }
}

/// One engine's door state. It lives with the engine's VM, not with its thread: NSH keeps one `Locals` per VM, hands it to
/// every call's `System` and closes it when the VM goes (nsh-js `system_locals`). So it cannot outlive its engine, whichever
/// thread the host runs the next engine on.
#[derive(Default)]
pub struct Engine {
    /// `Door.shutdown` ran on this engine (a quit, an update's restart).
    quit: std::cell::Cell<bool>,
    /// This engine has made its first call.
    entered: std::cell::Cell<bool>,
}

impl nsh_core::element::Local for Engine {}

/// What a door call made after `Door.shutdown` on the same engine answers.
pub const QUITTING: &str = "eBIRForms is closing.";

/// What an engine answers once a newer engine has booted on its thread while it was still alive. NSH runs one engine per
/// thread at a time, so nobody should read it.
pub const REPLACED: &str = "eBIRForms started again; this copy no longer answers.";

thread_local! {
    /// The engine whose door call runs now on this thread ([`enter`] sets it): what a boot records as the module's owner.
    static CALLER: RefCell<Option<Rc<Engine>>> = const { RefCell::new(None) };
}

/// Every door call that can boot goes through this first, with the calling engine's `System::locals`.
/// - Once this engine has quit, it is refused with [`QUITTING`]. A timer that fires between the quit and the process's end
///   cannot boot the app again and start a worker that sends.
/// - The first call of an engine on a thread where an earlier engine booted (a host that runs one engine after another on
///   one thread) ends that one first, as its thread's end would: its worker stops and its database closes (nothing is
///   left to do when it quit). Then this engine boots, with a launch and a worker of its own.
/// - An engine that has made calls and finds a newer engine's module on its thread (two VMs alive on one thread, which NSH
///   never runs) is refused with [`REPLACED`], never handed the newer one's module.
pub fn enter(engine: &nsh_core::element::Locals) -> Result<(), String> {
    let this = engine.get::<Engine>();
    if this.quit.get() {
        return Err(QUITTING.to_owned());
    }
    let owner = BOOTED.with(|slot| slot.borrow().as_ref().map(|booted| Rc::clone(&booted.owner)));
    if let Some(owner) = owner
        && !Rc::ptr_eq(&owner, &this)
    {
        if this.entered.get() {
            return Err(REPLACED.to_owned());
        }
        bir_app_api::bir_module::shutdown_at_engine_end();
        BOOTED.with(|slot| slot.borrow_mut().take());
    }
    this.entered.set(true);
    CALLER.with(|caller| *caller.borrow_mut() = Some(this));
    Ok(())
}

/// The module of this thread, booted on the first call, with the database guard checked against the
/// database path as it is now. The door's functions reach it after [`enter`]. Once the engine that booted it quit, it
/// answers no call, one that skipped [`enter`] included.
fn booted() -> Result<Rc<HostModule>, String> {
    let current = BOOTED.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|booted| (Rc::clone(&booted.module), booted.checked.clone(), booted.owner.quit.get()))
    });
    let Some((module, checked, quit)) = current else {
        return boot_thread();
    };
    if quit {
        return Err(QUITTING.to_owned());
    }
    let path = bir_core::db::app_database_path();
    if path != checked {
        if let Some(why) = guards::database_refusal_here(&path) {
            return Err(why);
        }
        BOOTED.with(|slot| {
            if let Some(booted) = slot.borrow_mut().as_mut() {
                booted.checked = path;
            }
        });
    }
    Ok(module)
}

/// The old host's launch, on this engine thread (the retired gpui shell's `src/host.rs`, `run`).
fn boot_thread() -> Result<Rc<HostModule>, String> {
    let process = process();
    let _serial = BOOT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    choose_database()?;
    // The database guard comes first: the account gate below already opens the database.
    let path = bir_core::db::app_database_path();
    if let Some(why) = guards::database_refusal_here(&path) {
        return Err(why);
    }
    // The guard is armed before anything opens the database or starts the worker, so a boot that fails after that
    // point (an inconsistent module, a panic) still stops the worker and closes the database at the thread's end. The
    // database slots are made first: the guard goes before them at the thread's end.
    bir_app_api::bir_module::prepare_engine();
    ARMED.with(|_| {});
    // Pauses filing, then opens the UI only when a saved session qualifies (or the dev bypass is on).
    bir_app_api::cloud_account::prepare_launch();
    if bir_app_api::account_gate::is_unlocked() {
        if process.send.worker_may_start() {
            // Opens the database and starts this engine's submission worker.
            bir_app_api::bir_module::open_database_at_launch();
        } else {
            // The send guard refused: the database opens, the worker does not exist.
            let _ = bir_app_api::bir_module::database_handle();
        }
    }
    bir_app_api::hooks().start_updater();
    let module = bir_app_api::module();
    module
        .validate()
        .map_err(|error| format!("The bir module is inconsistent: {error}"))?;
    let module = Rc::new(module);
    // The engine whose call boots it owns it; a call that skipped `enter` (a test's) boots one of its own.
    let owner = CALLER.with(|caller| caller.borrow_mut().take()).unwrap_or_default();
    BOOTED.with(|slot| {
        *slot.borrow_mut() = Some(Booted {
            module: Rc::clone(&module),
            checked: path,
            owner,
        });
    });
    Ok(module)
}

fn arguments(args: Vec<JsonValue>) -> HostArguments {
    HostArguments::new(args.into_iter().map(bir_app_api::bir_module::json))
}

/// Refuse what the send guard forbids, and say so. The sign-in screen's "session opened" is what would
/// start the submission worker outside the boot. The worker itself does not depend on this name: the latch set in
/// [`set_up`] keeps every route from starting it; this is the clear message.
fn permitted(name: &str) -> Result<(), String> {
    if name == "cloud_account_session_opened"
        && let SendMode::Refused(reason) = &process().send
    {
        return Err(format!("Not opening the session: {reason}"));
    }
    Ok(())
}

fn kind_of(module: &HostModule, name: &str, wanted: FunctionKind) -> Result<(), String> {
    if !module.has(name) {
        return Err(format!("There is no host function named {name}."));
    }
    let actual = if module.is_async(name) {
        FunctionKind::Async
    } else {
        FunctionKind::Sync
    };
    match (actual, wanted) {
        (FunctionKind::Async, FunctionKind::Sync) => Err(format!(
            "{name} is an async host function: call it with callAsync, not call."
        )),
        (FunctionKind::Sync, FunctionKind::Async) => Err(format!(
            "{name} is a sync host function: call it with call, not callAsync."
        )),
        _ => Ok(()),
    }
}

/// What a person reads when another copy of the app holds the database: it happens with the one-copy guard off (a test
/// environment, `BIR_SINGLE_INSTANCE=0`); with it on, the second copy hands over to the first and never gets here. The
/// sign-in lock's card shows it above its Try again, so its last sentence says what to do there. The holder can also be
/// an earlier engine of this same process that is still ending (a send or a calendar request it waits for), which is
/// why the sentence offers waiting: its end closes the database, and Try again then opens it.
pub const SECOND_COPY: &str =
    "eBIRForms still has this database open, in another window or one that is closing. Close it or wait a moment, then press Try again.";

/// Whether `message` is bir-core's own sentence for a database another process owns (`DbError::LiveDatabaseInUse`): the text
/// is read back from core, with a marker where the path goes, so it follows core if core's words change.
fn is_database_in_use(message: &str) -> bool {
    const MARK: &str = "\u{1}";
    let sample = bir_core::db::DbError::LiveDatabaseInUse(PathBuf::from(MARK)).to_string();
    let Some((head, tail)) = sample.split_once(MARK) else {
        return false;
    };
    message.len() >= head.len() + tail.len() && message.starts_with(head) && message.ends_with(tail)
}

/// A host function's error as the page is told it: the host's own sentence, unchanged, except for a second copy's. Core's
/// sentence for that names the painted app and a protocol (it is a developer's, and the old shell shows it as it is); here the
/// page gets [`SECOND_COPY`] and core's sentence, with the path, goes to the log.
fn said(message: &str) -> String {
    if is_database_in_use(message) {
        tracing::warn!("{message}");
        return SECOND_COPY.to_owned();
    }
    message.to_owned()
}

/// Call a synchronous host function.
pub fn call(name: &str, args: Vec<JsonValue>) -> Result<JsonValue, String> {
    let module = booted()?;
    kind_of(&module, name, FunctionKind::Sync)?;
    permitted(name)?;
    module
        .call(name, &arguments(args))
        .map(|value| bir_app_api::bir_module::to_json(&value))
        .map_err(|error| said(error.message()))
}

/// Call an asynchronous host function.
pub async fn call_async(name: String, args: Vec<JsonValue>) -> Result<JsonValue, String> {
    let module = booted()?;
    kind_of(&module, &name, FunctionKind::Async)?;
    permitted(&name)?;
    let future = module
        .begin(&name, &arguments(args))
        .map_err(|error| said(error.message()))?;
    future
        .await
        .map(|value| bir_app_api::bir_module::to_json(&value))
        .map_err(|error| said(error.message()))
}

/// Every host function with its kind, sorted by name. Needs no database.
pub fn functions() -> Vec<(String, FunctionKind)> {
    bir_app_api::module()
        .functions()
        .into_iter()
        .map(|(name, kind)| (name.to_owned(), kind))
        .collect()
}

/// Changes from this connection and from other connections, or unknown when SQLite cannot answer.
pub fn data_change_token() -> Result<Option<String>, String> {
    booted()?;
    let handle =
        bir_app_api::bir_module::database_handle().map_err(|error| said(error.message()))?;
    let db =
        bir_core::lock_timing::lock(&handle).map_err(|_| "database lock poisoned".to_owned())?;
    Ok(db
        .data_version()
        .map(|version| format!("{}:{version}", db.total_changes())))
}

/// The TypeScript declarations of the `bir` module, as the Rust modules wrote them.
pub fn declared_types() -> String {
    bir_app_api::module()
        .declared()
        .unwrap_or_default()
        .to_owned()
}

/// The first lines of `bir/types.d.ts`.
const TYPES_HEADER: &str = "// Generated from the Rust modules (the declaration blocks of `bir_app_api::module()`).\n\
// Do not edit. Regenerate with `nsh/bir/regenerate.sh`; a Rust test fails while this file is stale.\n";

/// The whole text of `nsh/bir/types.d.ts`: a header, then the `bir` module's declarations.
pub fn types_file() -> String {
    format!("{TYPES_HEADER}{}", declared_types())
}

/// How sending stands: the guard's verdict. Needs no database.
pub fn send_mode() -> &'static SendMode {
    &process().send
}

/// The database file this thread has open, or, while it has none, the one the environment names.
pub fn database_path() -> PathBuf {
    BOOTED
        .with(|slot| slot.borrow().as_ref().map(|booted| booted.checked.clone()))
        .unwrap_or_else(bir_core::db::app_database_path)
}

/// Stop the submission worker and close this engine's database (the old host's quit path). A second call does nothing.
/// Every later door call from this engine (`engine`, its `System::locals`) is refused with [`QUITTING`]: the app is
/// ending. A later engine is not refused, even on the same thread.
pub fn shutdown(engine: &nsh_core::element::Locals) {
    engine.get::<Engine>().quit.set(true);
    bir_app_api::bir_module::shutdown_for_quit();
    // The print previews' files (a filled return each) go with the run.
    crate::print::remove_all();
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use nsh_core::prelude::System;
    use serde_json::json;

    use super::*;

    /// Forgets the database name a test gave bir-core in-process when the test ends, even by a panic.
    struct ForgetDatabaseName;

    impl Drop for ForgetDatabaseName {
        fn drop(&mut self) {
            bir_core::db::set_database_path_override(None);
        }
    }

    /// The dev environment of the plan, a database nobody else opens, and a home folder of its own (the
    /// protected folders hang off it for the guard, on this thread).
    fn dev_env<R>(database: Option<&Path>, home: Option<&Path>, body: impl FnOnce() -> R) -> R {
        let database = database.map(|path| path.display().to_string());
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                ("EBIR_TEST_ENV", Some("1")),
                ("BIR_AUTH_BYPASS", Some("1")),
                ("BIR_SFTP_DRY_RUN", Some("1")),
                ("BIR_SFTP_LIVE", None),
                ("BIR_SFTP_HOST", None),
                ("BIR_DATABASE_PATH", database.as_deref()),
            ],
            || {
                let _forget = ForgetDatabaseName;
                bir_core::db::set_database_path_override(None);
                match home {
                    Some(home) => guards::with_home_for_test(home, body),
                    None => body(),
                }
            },
        )
    }

    fn empty() -> (tempfile::TempDir, PathBuf) {
        let folder = tempfile::tempdir().expect("temp folder");
        let database = folder.path().join("data").join("bir_data.db");
        (folder, database)
    }

    #[test]
    fn data_change_token_holds_for_reads_and_moves_for_local_and_external_writes() {
        let (folder, database) = empty();
        dev_env(Some(&database), Some(folder.path()), || {
            let first = data_change_token().expect("token").expect("known changes");
            call("get_setting", vec![json!("theme_preference")]).expect("read");
            assert_eq!(
                data_change_token().unwrap().as_deref(),
                Some(first.as_str())
            );
            // A local write to the database (the theme is a device setting since accounts-and-lock A1: it goes to
            // device.db, which this token does not follow).
            call("set_setting", vec![json!("theme_preference"), json!("dark")]).expect("device write");
            assert_eq!(data_change_token().unwrap().as_deref(), Some(first.as_str()));
            {
                let handle = bir_app_api::bir_module::database_handle().expect("handle");
                bir_core::lock_timing::lock(&handle)
                    .expect("lock")
                    .set_setting("door.local_write_probe", "1")
                    .expect("local write");
            }
            let second = data_change_token().unwrap().unwrap();
            assert_ne!(second, first);
            let other = bir_core::db::Database::open(&database).expect("another connection");
            let before_external = data_change_token().unwrap().unwrap();
            other.set_setting("theme_preference", "light").expect("external write");
            assert_ne!(data_change_token().unwrap().unwrap(), before_external);
        });
    }

    #[test]
    fn a_sync_call_reaches_the_host_and_a_host_error_keeps_its_message() {
        let (folder, database) = empty();
        dev_env(Some(&database), Some(folder.path()), || {
            let info = call("system_info", vec![]).expect("system_info");
            assert_eq!(info["database_path"], database.display().to_string());
            assert_eq!(info["app_version"], env!("CARGO_PKG_VERSION"));
            assert!(
                database.exists(),
                "the first call opened the empty database"
            );
            let refused = call("set_global_hotkey", vec![json!(1)]).unwrap_err();
            assert_eq!(refused, "argument 1 must be a string, got a number");
        });
    }

    /// The shared rule takes Shift and a key alone (the old shell registers it); NSH's listener does not, so `set_global_hotkey`
    /// refuses it when it is set (the hooks' `global_hotkey_registrable`: `NshHooks` asks `desktop::hotkey_shortcut`), with the
    /// host's sentence, and the stored combo stays as it was. (It used to be stored and never registered.) What NSH registers is
    /// stored (also the keys only NSH registers: space, the arrows, punctuation, F13 to F20), and clearing works.
    #[test]
    fn a_hotkey_this_listener_cannot_register_is_refused_when_it_is_set() {
        let (folder, database) = empty();
        dev_env(Some(&database), Some(folder.path()), || {
            let stored = || call("system_info", vec![]).expect("system_info")["global_hotkey"].clone();
            assert_eq!(stored(), JsonValue::Null);
            for combo in ["shift+E", "Shift+F12", " shift + 9 ", "banana+E"] {
                assert!(bir_app_api::hotkey_combo::is_registrable(combo.trim()), "{combo}");
                let refused = call("set_global_hotkey", vec![json!(combo)]).unwrap_err();
                assert_eq!(refused, bir_app_api::hotkey_combo::NOT_REGISTRABLE, "{combo}");
                assert_eq!(stored(), JsonValue::Null, "{combo}: nothing stored");
            }
            call("set_global_hotkey", vec![json!("control+alt+E")]).expect("registrable");
            assert_eq!(stored(), json!("control+alt+E"));
            let refused = call("set_global_hotkey", vec![json!("shift+E")]).unwrap_err();
            assert_eq!(refused, bir_app_api::hotkey_combo::NOT_REGISTRABLE);
            assert_eq!(stored(), json!("control+alt+E"), "the stored combo stays as it was");
            // F13 to F20 and the keys only NSH registers are stored; the old shell's rule would have refused them.
            for combo in ["control+F13", "alt+SPACE", "control+alt+LEFT", "cmd+COMMA", "control+alt+BACKTICK", "control+alt+F20"] {
                assert!(!bir_app_api::hotkey_combo::is_registrable(combo), "{combo}");
                call("set_global_hotkey", vec![json!(combo)]).unwrap_or_else(|error| panic!("{combo}: {error}"));
                assert_eq!(stored(), json!(combo), "{combo}");
            }
            // Not these: a key no listener of ours registers, a character, F21 (macOS keyboards end at F20), Shift with a NSH-only key.
            for combo in ["control+F21", "control+-", "control+TAB", "shift+SPACE", "control+space bar"] {
                let refused = call("set_global_hotkey", vec![json!(combo)]).unwrap_err();
                assert_eq!(refused, bir_app_api::hotkey_combo::NOT_REGISTRABLE, "{combo}");
            }
            assert_eq!(stored(), json!("control+alt+F20"), "a refusal leaves the stored combo");
            call("set_global_hotkey", vec![json!("")]).expect("cleared");
            assert_eq!(stored(), JsonValue::Null);
        });
    }

    /// A second copy on the same database (the one-copy guard is off in a test environment): the engine thread of another copy
    /// cannot take the owner lock, and the page is told in plain words, not in bir-core's developer sentence, which stays in the log.
    #[test]
    fn a_second_copy_on_the_same_database_is_told_in_plain_words() {
        let (folder, database) = empty();
        dev_env(Some(&database), Some(folder.path()), || {
            call("list_taxpayers", vec![]).expect("the first copy opens the database and holds its lock");
            // Another copy: another engine thread of this process (the door boots, and the database slot lives, per thread).
            let second = std::thread::scope(|scope| scope.spawn(|| call("list_taxpayers", vec![])).join().expect("the thread ran"));
            assert_eq!(second.unwrap_err(), SECOND_COPY);
            let asked = std::thread::scope(|scope| {
                scope
                    .spawn(|| {
                        let args = vec![json!("000123453"), json!("1601C"), json!(2025), json!(3), JsonValue::Null];
                        futures::executor::block_on(call_async("screen_form_open".into(), args))
                    })
                    .join()
                    .expect("the thread ran")
            });
            assert_eq!(asked.unwrap_err(), SECOND_COPY);
            // The first copy goes on as it was.
            call("list_taxpayers", vec![]).expect("the first copy still has it");
        });
    }

    /// Three engines in one process, one after the other (an Android relaunch in the process that outlived the last one):
    /// each engine thread opens the database through the door, and its end (the thread going, the door's guard) closes
    /// it, so the owner lock is free for the next.
    #[test]
    fn each_engine_end_frees_the_database_for_the_next() {
        let (folder, database) = empty();
        dev_env(Some(&database), Some(folder.path()), || {
            for engine in 1..=3 {
                std::thread::scope(|scope| {
                    scope
                        .spawn(|| call("list_taxpayers", vec![]).unwrap_or_else(|error| panic!("engine {engine}: {error}")))
                        .join()
                        .expect("the engine thread")
                });
                let lock = bir_core::db::try_acquire_live_owner_lock(&database);
                assert!(lock.is_ok(), "engine {engine}: the lock is free after its end");
            }
        });
    }

    /// The same in a process of its own, so its first engine is the process's first (as on a phone, where the
    /// process outlives a Back on Home and the next launch is a new engine in it): three engines one after the other
    /// each open the database and free it at their end, and then the locked card's Try again works over an engine
    /// that held the database and has ended. Before, the first engine's handle stayed held for the life of the
    /// process (the log-retention thread kept a clone of it, and only the first end of a process was handled).
    #[test]
    fn in_a_fresh_process_every_engine_end_frees_the_database() {
        const CHILD: &str = "NSH_DOOR_FRESH_PROCESS_ENGINES";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "door::tests::in_a_fresh_process_every_engine_end_frees_the_database", "--nocapture"])
                .env(CHILD, "1")
                .env("EBIR_TEST_ENV", "1")
                .env("BIR_AUTH_BYPASS", "1")
                .env("BUWIZ_NO_KEYCHAIN", "1")
                .env("BIR_SFTP_DRY_RUN", "1")
                .env_remove("BIR_SFTP_LIVE")
                .env_remove("BIR_SFTP_HOST")
                .env_remove("BIR_RELAY_URL")
                .env_remove("BUWIZ_RELAY_SEND")
                .status()
                .unwrap();
            assert!(status.success(), "the child process failed");
            return;
        }
        let (folder, database) = empty();
        dev_env(Some(&database), Some(folder.path()), || {
            for engine in 1..=3 {
                std::thread::scope(|scope| {
                    scope
                        .spawn(|| call("auth_state", vec![]).unwrap_or_else(|error| panic!("engine {engine}: {error}")))
                        .join()
                        .expect("the engine thread")
                });
                let lock = bir_core::db::try_acquire_live_owner_lock(&database);
                assert!(lock.is_ok(), "engine {engine}: the lock is free after its end");
            }
            let (opened, wait_for_open) = std::sync::mpsc::channel::<()>();
            let (end, wait_for_end) = std::sync::mpsc::channel::<()>();
            std::thread::scope(|scope| {
                let earlier = scope.spawn(move || {
                    call("auth_state", vec![]).expect("engine 4 opens the database");
                    opened.send(()).unwrap();
                    wait_for_end.recv().unwrap();
                });
                let later = scope.spawn(move || {
                    wait_for_open.recv().unwrap();
                    assert_eq!(call("auth_state", vec![]).unwrap_err(), SECOND_COPY, "engine 5 while 4 is open");
                    end.send(()).unwrap();
                    earlier.join().expect("engine 4 ended");
                    call("auth_state", vec![]).expect("engine 5: Try again opens the database");
                });
                // `join`, not the scope's end: it waits for the thread's locals, the door's guard among them.
                later.join().expect("engine 5 ended");
            });
            assert!(bir_core::db::try_acquire_live_owner_lock(&database).is_ok(), "free after engine 5");

            // From here on through the door's own functions (`crate::Door`, as NSH calls them), each engine with a
            // `System` of its own, as each VM has its own `Locals`.
            let auth_state = |engine: &System| crate::Door::call("auth_state".into(), vec![], engine.clone());

            // A quit (`Door.shutdown`) ends engine 6 for good. A door call it makes after that (a timer that fires before
            // the process ends: the shell's poll asks for the data token, the updater for its status) is refused and
            // boots nothing: no worker starts, and a return queued in the database meanwhile stays queued. Before, that
            // call booted the app again and the new worker's first tick sent the return.
            std::thread::scope(|scope| {
                scope
                    .spawn(|| {
                        let six = System::default();
                        auth_state(&six).expect("engine 6 boots");
                        assert!(worker_runs_here(), "engine 6: its own worker");
                        crate::Door::shutdown(six.clone());
                        assert!(!worker_runs_here(), "engine 6: quit stopped its worker");
                        assert!(bir_app_api::bir_module::owner_lock_released(), "engine 6: quit closed it");
                        queue_a_1601c(2024, 1);
                        assert_eq!(auth_state(&six).unwrap_err(), QUITTING, "a call after quit");
                        assert_eq!(crate::Door::data_change_token(six.clone()).unwrap_err(), QUITTING, "the poll after quit");
                        assert!(!worker_runs_here(), "no worker after quit");
                        // A worker's first tick takes a queued return at once; give one the time it would need.
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        assert_eq!(queued_1601c_status(2024, 1), "Queued", "the return queued after quit was not sent");
                    })
                    .join()
                    .expect("engine 6's thread")
            });
            assert!(bir_core::db::try_acquire_live_owner_lock(&database).is_ok(), "free after engine 6");

            // Two engines on ONE thread (a host that runs the next engine on the thread of the last). Engine 7 quits, and a
            // call it makes after that is refused. Engine 8's first call on the same thread boots again, with a worker of
            // its own that sends what was queued in between; engine 7 stays refused. Then engine 9 replaces engine 8 on
            // the thread without a quit (a VM the host dropped): its first call ends engine 8 as a thread's end would
            // (the worker stopped, the database closed) and boots with a database handle and a worker of its own.
            std::thread::scope(|scope| {
                scope
                    .spawn(|| {
                        let seven = System::default();
                        auth_state(&seven).expect("engine 7 boots");
                        assert!(worker_runs_here(), "engine 7: its own worker");
                        crate::Door::shutdown(seven.clone());
                        assert!(!worker_runs_here(), "engine 7: quit stopped its worker");
                        assert_eq!(auth_state(&seven).unwrap_err(), QUITTING, "engine 7 after its quit");
                        queue_a_1601c(2024, 2);

                        let eight = System::default();
                        auth_state(&eight).expect("engine 8 boots on the thread engine 7 quit on");
                        assert!(worker_runs_here(), "engine 8: a worker of its own");
                        wait_for_status_here(2024, 2, "Submitted", "engine 8's worker sends the 1601C queued before it");
                        assert_eq!(auth_state(&seven).unwrap_err(), QUITTING, "engine 7 stays refused");
                        // Only a weak reference: a strong one would hold the database open past engine 8's end.
                        let handle_of_eight = std::sync::Arc::downgrade(
                            &bir_app_api::bir_module::database_handle().expect("engine 8's database"),
                        );

                        let nine = System::default();
                        auth_state(&nine).expect("engine 9 boots in place of engine 8");
                        assert!(handle_of_eight.upgrade().is_none(), "engine 8's end closed its database");
                        assert!(worker_runs_here(), "engine 9: a worker of its own");
                        assert!(
                            crate::Door::data_change_token(nine.clone()).is_ok(),
                            "engine 9 answers after engine 8's end"
                        );
                    })
                    .join()
                    .expect("the thread of engines 7, 8 and 9")
            });
            assert!(bir_core::db::try_acquire_live_owner_lock(&database).is_ok(), "free after engine 9");

            // Two engines ALIVE together on one thread (NSH never runs that; the door does not rely on it). Engine 10
            // boots; engine 11's first call ends it and boots its own; engine 10's next call is refused, never handed
            // engine 11's module. After engine 11 quits, a call that skips the gate (the agent's listener boots with a
            // plain `door::call`) is refused, and so is the agent's start.
            std::thread::scope(|scope| {
                scope
                    .spawn(|| {
                        let ten = System::default();
                        auth_state(&ten).expect("engine 10 boots");
                        let eleven = System::default();
                        auth_state(&eleven).expect("engine 11 boots in its place");
                        assert_eq!(auth_state(&ten).unwrap_err(), REPLACED, "engine 10 gets nothing of engine 11's");
                        auth_state(&eleven).expect("engine 11 still answers");
                        crate::Door::shutdown(eleven.clone());
                        assert_eq!(call("system_info", vec![]).unwrap_err(), QUITTING, "a call past the gate, after the quit");
                        let agent = crate::agent::Agent::start(eleven.clone());
                        assert_eq!(agent.note.as_deref(), Some(QUITTING), "the agent starts nothing after the quit");
                        assert!(!agent.active);
                    })
                    .join()
                    .expect("the thread of engines 10 and 11")
            });
            assert!(bir_core::db::try_acquire_live_owner_lock(&database).is_ok(), "free after engine 11");
        });
    }

    fn worker_runs_here() -> bool {
        bir_app_api::bir_module::submission_worker_runs_for_this_engine()
    }

    /// Waits (20 s at most) for the demo taxpayer's 1601C of `year`/`month` to reach `status`, read through this
    /// engine's own database handle (the owner lock refuses a second open while it is open).
    fn wait_for_status_here(year: u16, month: u8, status: &str, what: &str) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let handle = bir_app_api::bir_module::database_handle().expect("this engine's database");
            let now = {
                let db = bir_core::lock_timing::lock(&handle).expect("the database lock");
                db.get_1601c_draft(&demo_profile().tin.full(), year, month)
                    .expect("read")
                    .map(|draft| format!("{:?}", draft.status))
            };
            if now.as_deref() == Some(status) {
                return;
            }
            assert!(std::time::Instant::now() < deadline, "{what}: still {now:?} after 20 s");
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    /// Juana Santos, TIN 000-123-453-00000, the demo taxpayer.
    fn demo_profile() -> bir_core::profile::TaxpayerProfile {
        serde_json::from_value(json!({
            "id": null,
            "full_name": "Juana Santos",
            "tin": {"segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000"},
            "rdo_code": "040",
            "line_of_business": "Retail",
            "registered_address": "1 Example Street, Pasig",
            "zip_code": "1600",
            "phone": "0280000000",
            "email": "juana@example.com",
            "default_form_type": "2551Qv2018",
            "taxpayer_type": "Individual",
            "business_start_date": "2010-01-01"
        }))
        .expect("the demo profile")
    }

    /// The demo taxpayer's 1601C for `year`/`month`, queued, written straight into the database (no engine has it open).
    fn queue_a_1601c(year: u16, month: u8) {
        let profile = demo_profile();
        let mut draft = bir_core::forms::form_1601c::Form1601CDraft::new_from_profile(&profile, year, month);
        draft.any_taxes_withheld = false;
        draft.transition_to_queued().expect("the demo 1601C queues");
        let (db, _, _) = bir_core::db::open_live_database().expect("the database, closed by the quit");
        if db.list_profiles().expect("the profiles").is_empty() {
            db.save_profile(profile).expect("the profile");
        }
        db.save_queued_1601c_draft(&draft).expect("the queued 1601C");
        db.close().expect("closed again");
    }

    fn queued_1601c_status(year: u16, month: u8) -> String {
        let (db, _, _) = bir_core::db::open_live_database().expect("the database");
        let draft = db
            .get_1601c_draft(&demo_profile().tin.full(), year, month)
            .expect("read")
            .expect("the 1601C row");
        let status = format!("{:?}", draft.status);
        db.close().expect("closed again");
        status
    }

    /// The sign-in lock's card ("eBIRForms is locked", the host's sentence, Try again; `AccessError.tsx`) over a
    /// database the engine before this one still has open: the sentence while it does, and Try again (the page asks
    /// `auth_state` again) opens it once that engine has ended.
    #[test]
    fn try_again_opens_the_database_once_the_engine_before_has_ended() {
        let (folder, database) = empty();
        dev_env(Some(&database), Some(folder.path()), || {
            let (opened, wait_for_open) = std::sync::mpsc::channel::<()>();
            let (end, wait_for_end) = std::sync::mpsc::channel::<()>();
            std::thread::scope(|scope| {
                let earlier = scope.spawn(move || {
                    call("auth_state", vec![]).expect("the earlier engine opens the database");
                    opened.send(()).unwrap();
                    wait_for_end.recv().unwrap();
                });
                wait_for_open.recv().unwrap();
                assert_eq!(call("auth_state", vec![]).unwrap_err(), SECOND_COPY);
                end.send(()).unwrap();
                earlier.join().expect("the earlier engine ended");
            });
            call("auth_state", vec![]).expect("Try again opens the database");
        });
    }

    /// The same card over a database another copy holds (a lock of its own on the file, as another process takes it):
    /// the sentence says what to do, Try again says it again while the other copy is open, and opens the database once
    /// it is closed.
    #[test]
    fn a_database_another_copy_holds_says_what_to_do_until_it_is_closed() {
        let (folder, database) = empty();
        dev_env(Some(&database), Some(folder.path()), || {
            let other = bir_core::db::try_acquire_live_owner_lock(&database).expect("the other copy");
            let said = call("auth_state", vec![]).unwrap_err();
            assert_eq!(said, SECOND_COPY);
            assert!(said.ends_with("Close it or wait a moment, then press Try again."), "{said}");
            assert_eq!(call("auth_state", vec![]).unwrap_err(), SECOND_COPY, "Try again while it is open");
            drop(other);
            call("auth_state", vec![]).expect("Try again once it is closed");
        });
    }

    /// Only bir-core's sentence for a database another process owns is replaced, whatever path it names; every other
    /// error, even one that says "already open", reaches the page as the host wrote it.
    #[test]
    fn only_the_second_copy_sentence_is_said_in_other_words() {
        for path in ["/tmp/a/bir_data.db", "C:\\Users\\x y\\bir_data.db", "/data/with space/bir_data.db", ""] {
            let core = bir_core::db::DbError::LiveDatabaseInUse(PathBuf::from(path)).to_string();
            assert!(core.contains("Op::Yield"), "the developer's sentence: {core}");
            assert_eq!(said(&core), SECOND_COPY, "{path:?}");
        }
        for other in [
            "The database is already open on this thread; call useDatabase before anything else.",
            "Refusing to open the owner's data folder.",
            "The database is in use elsewhere, so its recent changes could not be written into the database file (x).",
            "live database",
            "",
        ] {
            assert_eq!(said(other), other);
        }
        assert!(!SECOND_COPY.contains("painted") && !SECOND_COPY.contains("Op::Yield"));
    }

    #[test]
    fn a_call_with_the_wrong_kind_or_an_unknown_name_is_a_clear_error() {
        let (folder, database) = empty();
        dev_env(Some(&database), Some(folder.path()), || {
            let error = call("import_archive", vec![]).unwrap_err();
            assert_eq!(
                error,
                "import_archive is an async host function: call it with callAsync, not call."
            );
            let error =
                futures::executor::block_on(call_async("system_info".into(), vec![])).unwrap_err();
            assert_eq!(
                error,
                "system_info is a sync host function: call it with call, not callAsync."
            );
            let error = call("no_such_function", vec![]).unwrap_err();
            assert_eq!(error, "There is no host function named no_such_function.");
        });
    }

    #[test]
    fn an_async_call_runs_and_its_host_error_keeps_its_message() {
        let (folder, database) = empty();
        dev_env(Some(&database), Some(folder.path()), || {
            // Refused before anything opens: the page may not choose the file.
            let error = futures::executor::block_on(call_async(
                "import_archive".into(),
                vec![json!("x.zip")],
            ))
            .unwrap_err();
            assert_eq!(
                error,
                "import_archive takes no path; the host opens the file dialog"
            );
            // No engine runs here, so there is no UI to open the file dialog: said, not faked.
            let error = futures::executor::block_on(call_async("import_archive".into(), vec![]))
                .unwrap_err();
            assert_eq!(error, "no app is running to open the file dialog");
            // An unknown taxpayer is the host's refusal, its words unchanged.
            let error = futures::executor::block_on(call_async(
                "screen_form_open".into(),
                vec![json!("999999999"), json!("1601C"), json!(2025), json!(1)],
            ))
            .unwrap_err();
            assert!(!error.is_empty());
            // And one that works: the demo taxpayer's 1601C opens.
            call(
                "create_taxpayer",
                vec![json!({
                    "tin_root": "000-123-453",
                    "name": "Juana Santos",
                    "taxpayer_type": "Individual",
                    "head_office": {"rdo_code": "039", "registered_address": "Quezon City", "zip_code": "1100"}
                })],
            )
            .expect("create_taxpayer");
            let opened = futures::executor::block_on(call_async(
                "screen_form_open".into(),
                vec![
                    json!("00012345300000"),
                    json!("1601C"),
                    json!(2025),
                    json!(1),
                ],
            ))
            .expect("screen_form_open");
            assert_eq!(opened["form_code"], "1601C");
            assert_eq!(opened["status"], "Draft");
        });
    }

    #[test]
    fn a_database_in_the_owners_folders_is_refused_before_it_is_created() {
        let home = tempfile::tempdir().expect("home");
        for relative in [
            "Library/Group Containers/group.dev.goldcoders.bir",
            "BuwizNsh/data",
            "BuwizNsh/migration",
        ] {
            let database = home.path().join(relative).join("bir_data.db");
            dev_env(Some(&database), Some(home.path()), || {
                for _ in 0..2 {
                    let error = call("system_info", vec![]).unwrap_err();
                    assert!(error.starts_with("Refusing to open"), "{error}");
                }
                let error =
                    futures::executor::block_on(call_async("import_archive".into(), vec![]))
                        .unwrap_err();
                assert!(error.starts_with("Refusing to open"), "{error}");
            });
            assert!(!database.exists(), "{relative}: nothing was created");
            assert!(
                !database.parent().unwrap().exists(),
                "{relative}: no folder either"
            );
        }
    }

    #[test]
    fn a_test_environment_with_no_database_named_gets_a_scratch_one() {
        let home = tempfile::tempdir().expect("home");
        dev_env(None, Some(home.path()), || {
            let info = call("system_info", vec![]).expect("system_info");
            let path = PathBuf::from(info["database_path"].as_str().expect("path"));
            assert!(
                path.starts_with(std::env::temp_dir().join("ebirforms-door")),
                "{path:?}"
            );
            assert!(path.exists());
            assert!(guards::database_refusal_here(&path).is_none());
            assert_eq!(
                std::env::var_os("BIR_DATABASE_PATH"),
                None,
                "the scratch database is named in-process, the environment is not changed"
            );
        });
    }

    #[test]
    fn use_database_names_the_file_before_the_first_call_only() {
        let (folder, database) = empty();
        dev_env(None, Some(folder.path()), || {
            assert!(use_database("").is_err());
            let protected = folder.path().join("BuwizNsh/data/bir_data.db");
            let error = use_database(&protected.display().to_string()).unwrap_err();
            assert!(error.starts_with("Refusing to open"), "{error}");
            assert!(!protected.exists());
            assert_eq!(
                use_database(&database.display().to_string()).expect("named"),
                database.display().to_string()
            );
            assert_eq!(
                std::env::var_os("BIR_DATABASE_PATH"),
                None,
                "the named database is given to bir-core in-process, the environment is not changed"
            );
            let info = call("system_info", vec![]).expect("system_info");
            assert_eq!(info["database_path"], database.display().to_string());
            let late = use_database(&database.display().to_string()).unwrap_err();
            assert!(late.contains("already open"), "{late}");
        });
        temp_env::with_var("EBIR_TEST_ENV", None::<&str>, || {
            let error = use_database("/tmp/whatever.db").unwrap_err();
            assert!(error.contains("test environments"), "{error}");
        });
    }

    #[test]
    fn the_host_functions_are_listed_with_their_kinds() {
        let functions = functions();
        let kind = |wanted: &str| {
            functions
                .iter()
                .find(|(name, _)| name == wanted)
                .map(|(_, kind)| *kind)
        };
        assert_eq!(kind("system_info"), Some(FunctionKind::Sync));
        assert_eq!(kind("import_archive"), Some(FunctionKind::Async));
        assert_eq!(kind("screen_form_open"), Some(FunctionKind::Async));
        assert_eq!(kind("not_a_function"), None);
        assert!(
            functions.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "sorted"
        );
        assert!(declared_types().contains("export function system_info"));
    }

    /// `nsh/bir/types.d.ts` is committed; when the Rust declarations change it must be regenerated.
    #[test]
    fn the_committed_types_file_is_current() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("bir/types.d.ts");
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == types_file(),
            "{} is stale: run nsh/bir/regenerate.sh",
            path.display()
        );
    }
    /// `EBIR_TEST_ENV` reads the way bir-core reads it. (The door used to take any value but `0`, `false`,
    /// `no` and `off`, so `enabled` or `2` made a test run that bir-core does not treat as one.)
    #[test]
    fn the_door_reads_test_environment_the_way_bir_core_does() {
        for value in [
            None,
            Some(""),
            Some("1"),
            Some("true"),
            Some(" YES "),
            Some("on"),
            Some("0"),
            Some("false"),
            Some("no"),
            Some("off"),
            Some("2"),
            Some("enabled"),
            Some("anything"),
        ] {
            temp_env::with_var("EBIR_TEST_ENV", value, || {
                assert_eq!(
                    test_environment(),
                    bir_core::db::test_environment(),
                    "{value:?}"
                );
            });
        }
        for refused in ["enabled", "2", "anything", ""] {
            temp_env::with_var("EBIR_TEST_ENV", Some(refused), || {
                assert!(!test_environment(), "{refused:?}");
            });
        }
        for accepted in ["1", "true", "yes", "on"] {
            temp_env::with_var("EBIR_TEST_ENV", Some(accepted), || {
                assert!(test_environment(), "{accepted:?}");
            });
        }
    }

    /// The frame's polls are asked for only in a test environment, and only by `1`.
    #[test]
    fn the_shell_poll_switch_is_read_only_in_a_test_environment() {
        for (test_env, poll, expected) in [
            (Some("1"), Some("1"), true),
            (Some("1"), None, false),
            (Some("1"), Some("0"), false),
            (Some("1"), Some("true"), false),
            (None, Some("1"), false),
        ] {
            temp_env::with_vars([("EBIR_TEST_ENV", test_env), (SHELL_POLL, poll)], || {
                assert_eq!(shell_poll(), expected, "{test_env:?} {poll:?}");
            });
        }
    }

    /// A debug build switches the keychain off whatever the caller set and whether or not this is a test
    /// run; a release build leaves the caller's variable alone.
    #[test]
    #[cfg(not(target_os = "android"))]
    fn a_debug_build_always_switches_the_keychain_off_and_a_release_build_leaves_it() {
        for caller in [None, Some(""), Some("0"), Some("false"), Some("1")] {
            temp_env::with_vars(
                [("BUWIZ_NO_KEYCHAIN", caller), ("EBIR_TEST_ENV", None)],
                || {
                    switch_off_keychain(true);
                    assert_eq!(
                        std::env::var("BUWIZ_NO_KEYCHAIN").as_deref(),
                        Ok("1"),
                        "debug, caller {caller:?}"
                    );
                    assert!(bir_core::platform::keychain_disabled());
                },
            );
            temp_env::with_vars(
                [("BUWIZ_NO_KEYCHAIN", caller), ("EBIR_TEST_ENV", None)],
                || {
                    switch_off_keychain(false);
                    assert_eq!(
                        std::env::var("BUWIZ_NO_KEYCHAIN").ok().as_deref(),
                        caller,
                        "release, caller {caller:?}"
                    );
                },
            );
        }
    }

    /// Registering the native module is what sets the process up, before anything is called and before any
    /// other thread exists: it needs a process of its own, where nothing has run yet.
    #[test]
    fn registration_prepares_the_process_before_anything_is_called() {
        const CHILD: &str = "NSH_DOOR_REGISTRATION_CHILD";
        if std::env::var_os(CHILD).is_some() {
            assert!(!process_prepared(), "nothing has run yet");
            let _ = crate::registry();
            assert!(process_prepared(), "the registration set the process up");
            assert_eq!(std::env::var("BIR_SFTP_DRY_RUN").as_deref(), Ok("1"));
            assert_eq!(std::env::var_os("BIR_SFTP_LIVE"), None);
            assert!(bir_core::platform::keychain_disabled());
            return;
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "door::tests::registration_prepares_the_process_before_anything_is_called",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env_remove("BUWIZ_NO_KEYCHAIN")
            .env_remove("BIR_SFTP_DRY_RUN")
            .env_remove("BIR_SFTP_HOST")
            .env_remove("EBIR_TEST_ENV")
            .env("BIR_SFTP_LIVE", "1")
            .status()
            .unwrap();
        assert!(status.success(), "the registration child failed");
    }

    /// The process set-up latches the worker off in bir-app-api when the send guard refuses, and not when it allows a
    /// dry run. The test runs `set_up`, the body `process` runs, with only the guard's verdict given: the environment
    /// cannot make a refusal (the send guard forces a dry run unless the mock listens). Each case in a process of its own:
    /// the latch is one way and process-wide.
    #[test]
    fn a_refused_send_guard_latches_the_worker_off() {
        const CHILD: &str = "NSH_DOOR_WORKER_LATCH_CHILD";
        match std::env::var(CHILD).as_deref() {
            Ok("refused") => {
                let process = set_up(|| SendMode::Refused("a reason".into()));
                assert!(!process.send.worker_may_start());
                assert!(bir_app_api::bir_module::submission_worker_refused(), "refused: latched");
                return;
            }
            Ok("allowed") => {
                let process = set_up(|| SendMode::DryRun);
                assert!(process.send.worker_may_start());
                assert!(!bir_app_api::bir_module::submission_worker_refused(), "a dry run: no latch");
                return;
            }
            _ => {}
        }
        for case in ["refused", "allowed"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "door::tests::a_refused_send_guard_latches_the_worker_off", "--nocapture"])
                .env(CHILD, case)
                .status()
                .unwrap();
            assert!(status.success(), "the {case} child failed");
        }
    }

    /// Set the modified time of a file or folder to `by` ago.
    fn grow_old(path: &Path, by: std::time::Duration) {
        let when = std::time::SystemTime::now() - by;
        if path.is_dir() {
            // Windows opens a folder only with backup semantics, and setting its time takes write-attributes access.
            #[cfg(windows)]
            let folder = {
                use std::os::windows::fs::OpenOptionsExt;
                use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS, FILE_WRITE_ATTRIBUTES};
                std::fs::OpenOptions::new()
                    .access_mode(FILE_WRITE_ATTRIBUTES)
                    .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                    .open(path)
                    .unwrap()
            };
            #[cfg(not(windows))]
            let folder = std::fs::File::open(path).unwrap();
            folder.set_modified(when).unwrap();
        } else {
            // On Windows asking only to write attributes, which a file another handle holds unshared (an owner lock)
            // still allows.
            #[cfg(windows)]
            let file = {
                use std::os::windows::fs::OpenOptionsExt;
                std::fs::OpenOptions::new()
                    .access_mode(windows_sys::Win32::Storage::FileSystem::FILE_WRITE_ATTRIBUTES)
                    .open(path)
                    .unwrap()
            };
            #[cfg(not(windows))]
            let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
            file.set_modified(when).unwrap();
        }
    }

    const LONG_AGO: std::time::Duration = std::time::Duration::from_secs(3 * 24 * 60 * 60);

    /// A scratch folder with a database, its log and (after a lock) its lock file, all of them `by` old.
    /// A process id comes round again: the folder a dead run left under it, database and all, is emptied before the
    /// new run uses it.
    #[test]
    fn a_scratch_folder_left_under_a_reused_process_id_starts_empty() {
        let root = tempfile::tempdir().expect("scratch root");
        let left = root.path().join("4242-0");
        std::fs::create_dir_all(&left).unwrap();
        std::fs::write(left.join("bir_data.db"), b"a dead run's database, with its PIN").unwrap();
        let folder = fresh_scratch_folder(root.path(), 4242, 0).expect("fresh folder");
        assert_eq!(folder, left);
        assert!(folder.is_dir());
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0, "nothing of the dead run is left");
        // Another engine of the same process, and a first run, get folders of their own.
        assert!(fresh_scratch_folder(root.path(), 4242, 1).expect("second").ends_with("4242-1"));
        assert!(fresh_scratch_folder(root.path(), 7, 0).expect("new").is_dir());
    }

    fn scratch_folder(root: &Path, name: &str, by: std::time::Duration) -> PathBuf {
        let folder = root.join(name);
        std::fs::create_dir_all(&folder).unwrap();
        for file in ["bir_data.db", "bir_data.db-wal"] {
            std::fs::write(folder.join(file), b"scratch").unwrap();
        }
        age_everything(&folder, by);
        folder
    }

    fn age_everything(folder: &Path, by: std::time::Duration) {
        for entry in std::fs::read_dir(folder).unwrap() {
            grow_old(&entry.unwrap().path(), by);
        }
        grow_old(folder, by);
    }

    #[test]
    fn an_old_quiet_scratch_folder_is_forgotten_and_a_recent_one_is_not() {
        let root = tempfile::tempdir().expect("scratch root");
        let old = scratch_folder(root.path(), "1-0", LONG_AGO);
        let recent = scratch_folder(root.path(), "2-0", std::time::Duration::from_secs(60));
        let stray = root.path().join("notes.txt");
        std::fs::write(&stray, b"not a scratch folder").unwrap();
        grow_old(&stray, LONG_AGO);

        forget_old_scratch(root.path(), SCRATCH_LIFETIME);

        assert!(!old.exists(), "a folder nobody changed for days");
        assert!(recent.is_dir(), "a recent folder");
        assert!(stray.is_file(), "only folders are ever deleted");
    }

    /// A folder's own modified time moves only when a file is added or removed in it, not while the database
    /// inside is written: a live run's folder looks days old. It is the newest change inside that counts.
    #[test]
    fn a_scratch_folder_whose_database_was_written_lately_is_kept() {
        let root = tempfile::tempdir().expect("scratch root");
        let live = scratch_folder(root.path(), "3-0", LONG_AGO);
        // The run writes its database: the file is new, the folder is as old as ever.
        std::fs::write(live.join("bir_data.db"), b"a commit a minute ago").unwrap();
        grow_old(&live, LONG_AGO);

        forget_old_scratch(root.path(), SCRATCH_LIFETIME);

        assert!(live.join("bir_data.db").is_file(), "a live run's database");
    }

    /// A run that has its database open holds the owner lock, however quiet it has been: its folder stays.
    /// Once the lock is gone the folder goes.
    #[test]
    fn a_scratch_folder_whose_owner_lock_is_held_is_kept() {
        let root = tempfile::tempdir().expect("scratch root");
        let folder = scratch_folder(root.path(), "4-0", LONG_AGO);
        let lock = bir_core::db::try_acquire_live_owner_lock(&folder.join("bir_data.db"))
            .expect("a free lock");
        // Taking the lock made its file; age everything again so only the lock is left to protect it.
        age_everything(&folder, LONG_AGO);

        forget_old_scratch(root.path(), SCRATCH_LIFETIME);
        assert!(folder.join("bir_data.db").is_file(), "held by a live run");

        drop(lock);
        // A test beside this one may be starting a child process right now: until that child execs it shares
        // our open lock file, so on Unix the lock outlives the drop for an instant. Try again past it.
        for _ in 0..100 {
            forget_old_scratch(root.path(), SCRATCH_LIFETIME);
            if !folder.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!folder.exists(), "nobody holds it any more");
    }

    /// A release build that runs in a test environment (`EBIR_TEST_ENV`, read as bir-core reads it) switches
    /// the keychain off too: before, only a debug build did, so a release-build test run could reach the OS
    /// keychain. Outside a test environment a release build still leaves the caller's variable alone.
    #[test]
    #[cfg(not(target_os = "android"))]
    fn a_release_build_in_a_test_environment_switches_the_keychain_off() {
        for caller in [None, Some(""), Some("0"), Some("false"), Some("1")] {
            for test_run in ["1", "true", "yes", "on", " YES "] {
                temp_env::with_vars(
                    [
                        ("BUWIZ_NO_KEYCHAIN", caller),
                        ("EBIR_TEST_ENV", Some(test_run)),
                    ],
                    || {
                        switch_off_keychain(false);
                        assert_eq!(
                            std::env::var("BUWIZ_NO_KEYCHAIN").as_deref(),
                            Ok("1"),
                            "release, test run {test_run:?}, caller {caller:?}"
                        );
                        assert!(bir_core::platform::keychain_disabled());
                    },
                );
            }
            // A value bir-core does not treat as a test run is not one here either.
            for not_a_test_run in [None, Some(""), Some("0"), Some("off"), Some("enabled")] {
                temp_env::with_vars(
                    [
                        ("BUWIZ_NO_KEYCHAIN", caller),
                        ("EBIR_TEST_ENV", not_a_test_run),
                    ],
                    || {
                        switch_off_keychain(false);
                        assert_eq!(
                            std::env::var("BUWIZ_NO_KEYCHAIN").ok().as_deref(),
                            caller,
                            "release, no test run ({not_a_test_run:?}), caller {caller:?}"
                        );
                    },
                );
            }
        }
    }

    /// A scratch folder goes while its owner lock is still held: a run that starts to open the database
    /// during the removal is refused, never handed a folder that is being emptied. (Before, the lock was
    /// dropped first and the folder removed after.)
    #[test]
    fn a_scratch_folder_is_removed_while_its_owner_lock_is_held() {
        let root = tempfile::tempdir().expect("scratch root");
        let folder = scratch_folder(root.path(), "5-0", LONG_AGO);
        let seen = std::cell::RefCell::new(Vec::new());

        forget_old_scratch_with(root.path(), SCRATCH_LIFETIME, |doomed| {
            seen.borrow_mut().push(doomed.to_path_buf());
            let rival = bir_core::db::try_acquire_live_owner_lock(&doomed.join("bir_data.db"));
            assert!(
                matches!(rival, Err(bir_core::db::DbError::LiveDatabaseInUse(_))),
                "another run could take the owner lock while the folder was being removed: {rival:?}"
            );
            let _ = std::fs::remove_dir_all(doomed);
        });

        assert_eq!(
            *seen.borrow(),
            std::slice::from_ref(&folder),
            "the quiet folder was the one removed"
        );
        assert!(!folder.exists(), "and it is gone");
    }

    /// Two engines in one process each name their own scratch database. The second boot used to repoint the
    /// first's too (the name was one for the whole process), so the first engine then answered with the
    /// second's file.
    #[test]
    fn a_second_engine_booting_does_not_repoint_the_first() {
        let home = tempfile::tempdir().expect("home");
        dev_env(None, Some(home.path()), || {
            let engine =
                |boot_second: std::sync::mpsc::Receiver<()>,
                 first_boot: std::sync::mpsc::Sender<String>,
                 after: std::sync::mpsc::Sender<(String, String)>| {
                    std::thread::spawn(move || {
                        let path = |what: &str| {
                            let info = call("system_info", vec![]).expect(what);
                            info["database_path"].as_str().expect("path").to_owned()
                        };
                        let before = path("the first boot");
                        first_boot.send(before.clone()).unwrap();
                        boot_second.recv().unwrap();
                        after.send((before, path("a later call"))).unwrap();
                    })
                };
            let (go_tx, go_rx) = std::sync::mpsc::channel();
            let (booted_tx, booted_rx) = std::sync::mpsc::channel();
            let (after_tx, after_rx) = std::sync::mpsc::channel();
            let first = engine(go_rx, booted_tx, after_tx);
            let first_path = booted_rx.recv().unwrap();

            // The second engine boots now, on a thread of its own.
            let second_path = std::thread::spawn(|| {
                let info = call("system_info", vec![]).expect("the second boot");
                info["database_path"].as_str().expect("path").to_owned()
            })
            .join()
            .unwrap();
            go_tx.send(()).unwrap();
            let (before, later) = after_rx.recv().unwrap();
            first.join().unwrap();

            assert_ne!(
                first_path, second_path,
                "two engines, two scratch databases"
            );
            assert_eq!(before, first_path);
            assert_eq!(
                later, first_path,
                "the second engine's boot repointed the first one's database"
            );
        });
    }
}
