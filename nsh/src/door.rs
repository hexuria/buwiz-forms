//! The door: the Rust brain's host functions (`bir_app_api::module()`, the module named `bir`) behind the
//! NSH native module. This file is the plain-Rust half (boot, guards, dispatch); `lib.rs` is its TSX face.
//!
//! * One `HostModule` per engine thread. It holds `Rc` and the database slot is thread-local, so every call
//!   runs on the engine thread, as the old shell's calls ran on its main thread.
//! * The first call on a thread boots it, in the old host's order: hooks, send guard, database guard, the
//!   account gate (`cloud_account::prepare_launch`), then the database and the submission worker.
//! * The guards are the contract's ("The door" in `docs/plans/nsh-tsx-migration.md`): a dry run is forced
//!   before anything opens, and no database inside the owner's folders is ever opened.

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
/// **This is the only place the door changes the process environment** (`BUWIZ_NO_KEYCHAIN`,
/// `BIR_SFTP_DRY_RUN`, `BIR_SFTP_LIVE`; the database a test run opens is named with
/// `bir_core::db::set_database_path_override`, not the environment). It runs at the registration of the native
/// module (`lib.rs`): both generated hosts (`nsh/.nsh/rust/macos-host/main.rs`, `.../test/main.rs`) build the
/// registry as the first statement of `main`, before NSH starts any thread, and the database, the submission
/// worker and every reader thread of ours come later still. So no thread of ours can be reading the
/// environment while it changes (NSH's own startup follows the registry, and it reads nothing named `BIR_*`),
/// and every later reader sees the finished values. Should the registry ever be built later (or never), the
/// first host call runs this instead, before it opens anything.
fn process() -> &'static Process {
    PROCESS.get_or_init(|| set_up(guards::send_guard))
}

/// The set-up [`process`] runs, with the send guard given (production passes `guards::send_guard`; a test passes one
/// that refuses, which the environment alone cannot make). A refused send guard holds where the submission worker
/// starts (bir-app-api's latch), so no route can start it, whatever host function it comes through. The name check in
/// [`permitted`] stays, for the clear message.
fn set_up(send_guard: impl FnOnce() -> SendMode) -> Process {
    switch_off_keychain(cfg!(debug_assertions));
    let _ = bir_app_api::install_hooks(NshHooks);
    let send = send_guard();
    if !send.worker_may_start() {
        bir_app_api::bir_module::refuse_submission_worker();
    }
    Process { send }
}

/// Set up the process now. The native module's registration calls it (see [`process`]).
pub(crate) fn prepare_process() {
    process();
}

/// Whether [`prepare_process`] has run in this process.
#[cfg(test)]
pub(crate) fn process_prepared() -> bool {
    PROCESS.get().is_some()
}

/// Debug builds never reach the OS keychain, whatever the caller set: `BUWIZ_NO_KEYCHAIN=1` before anything
/// in bir-core runs. So does any build running in a test environment (`EBIR_TEST_ENV`, read as bir-core reads
/// it): a release build under a test must not reach the keychain either. A release build outside a test
/// environment leaves the variable to the caller.
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
    let root = std::env::temp_dir().join("ebirforms-door");
    SWEPT.call_once(|| forget_old_scratch(&root, SCRATCH_LIFETIME));
    let folder = root.join(format!(
        "{}-{}",
        std::process::id(),
        SCRATCH_SEQUENCE.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("Cannot make a scratch database folder: {error}"))?;
    bir_core::db::set_database_path_override(Some(folder.join("bir_data.db")));
    Ok(())
}

/// Whether the door makes a scratch database of its own (per engine thread, in [`choose_database`]): a test environment
/// whose caller named none (`BIR_DATABASE_PATH` unset or empty). A test's [`use_database`] names one later, per thread.
/// The one-copy guard asks this at registration, before any thread has named anything.
pub(crate) fn scratch_database_wanted() -> bool {
    test_environment() && !std::env::var_os(DATABASE_PATH).is_some_and(|value| !value.is_empty())
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
}

thread_local! {
    static BOOTED: RefCell<Option<Booted>> = const { RefCell::new(None) };
    /// Dropped when the engine thread ends. Touched after the database slot exists, so it is destroyed
    /// before the slot is (thread locals go in reverse order of their first use).
    static ARMED: ShutdownGuard = const { ShutdownGuard };
}

/// Stops the submission worker and closes the database when the engine thread goes (the quit path of the
/// old host: `shutdown_for_quit`). `Door.shutdown` does the same on request.
struct ShutdownGuard;

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        let _ = std::panic::catch_unwind(bir_app_api::bir_module::shutdown_for_quit);
    }
}

/// The module of this thread, booted on the first call, with the database guard checked against the
/// database path as it is now.
fn booted() -> Result<Rc<HostModule>, String> {
    let current = BOOTED.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|booted| (Rc::clone(&booted.module), booted.checked.clone()))
    });
    let Some((module, checked)) = current else {
        return boot_thread();
    };
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

/// The old host's launch, on this engine thread (`crates/bir-shell/src/host.rs`, `run`).
fn boot_thread() -> Result<Rc<HostModule>, String> {
    let process = process();
    let _serial = BOOT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    choose_database()?;
    // The database guard comes first: the account gate below already opens the database.
    let path = bir_core::db::app_database_path();
    if let Some(why) = guards::database_refusal_here(&path) {
        return Err(why);
    }
    // Pauses filing, then opens the UI only when a saved session qualifies (or the dev bypass is on).
    bir_app_api::cloud_account::prepare_launch();
    if bir_app_api::account_gate::is_unlocked() {
        if process.send.worker_may_start() {
            // Opens the database and starts the submission worker, once per process.
            bir_app_api::bir_module::open_database_at_launch();
        } else {
            // The send guard refused: the database opens, the worker does not exist.
            let _ = bir_app_api::bir_module::database_handle();
        }
        bir_app_api::hooks().start_updater();
    }
    let module = bir_app_api::module();
    module
        .validate()
        .map_err(|error| format!("The bir module is inconsistent: {error}"))?;
    let module = Rc::new(module);
    ARMED.with(|_| {});
    BOOTED.with(|slot| {
        *slot.borrow_mut() = Some(Booted {
            module: Rc::clone(&module),
            checked: path,
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
/// environment, `BIR_SINGLE_INSTANCE=0`); with it on, the second copy hands over to the first and never gets here.
pub const SECOND_COPY: &str = "eBIRForms is already open on this database. Close the other window and try again.";

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

/// Stop the submission worker and close the database (the old host's quit path). A second call does nothing.
pub fn shutdown() {
    bir_app_api::bir_module::shutdown_for_quit();
    // The print previews' files (a filled return each) go with the run.
    crate::print::remove_all();
}

#[cfg(test)]
mod tests {
    use std::path::Path;

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
        forget_old_scratch(root.path(), SCRATCH_LIFETIME);
        assert!(!folder.exists(), "nobody holds it any more");
    }

    /// A release build that runs in a test environment (`EBIR_TEST_ENV`, read as bir-core reads it) switches
    /// the keychain off too: before, only a debug build did, so a release-build test run could reach the OS
    /// keychain. Outside a test environment a release build still leaves the caller's variable alone.
    #[test]
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
