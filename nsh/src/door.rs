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
use std::ffi::OsString;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use bir_app_api::host::{FunctionKind, HostArguments, HostModule};
use nsh_core::JsonValue;

use crate::guards::{self, SendMode};
use crate::hooks::NshHooks;

/// What is decided once for the whole process.
struct Process {
    send: SendMode,
}

static PROCESS: OnceLock<Process> = OnceLock::new();

/// Install the hooks and run the send guard, once. Nothing else has happened yet: no database is open and
/// no worker exists.
fn process() -> &'static Process {
    PROCESS.get_or_init(|| {
        let _ = bir_app_api::install_hooks(NshHooks);
        if test_environment() {
            // SAFETY: once, at boot, before the database and the worker exist.
            unsafe { std::env::set_var("BUWIZ_NO_KEYCHAIN", "1") };
        }
        Process {
            send: guards::send_guard(),
        }
    })
}

/// `EBIR_TEST_ENV` is on: a dev, test or agent run. The keychain stays shut, and a database nobody named
/// is a scratch one.
pub fn test_environment() -> bool {
    std::env::var("EBIR_TEST_ENV").is_ok_and(|value| {
        let value = value.trim().to_ascii_lowercase();
        !value.is_empty() && !matches!(value.as_str(), "0" | "false" | "no" | "off")
    })
}

const DATABASE_PATH: &str = "BIR_DATABASE_PATH";

/// The scratch database path this process last made, so a later thread can tell it from a path someone
/// named.
static SCRATCH: Mutex<Option<OsString>> = Mutex::new(None);
static SCRATCH_SEQUENCE: AtomicU32 = AtomicU32::new(0);
/// One boot at a time: the environment names the database, and a boot reads it right after.
static BOOT: Mutex<()> = Mutex::new(());

/// In a test environment with no `BIR_DATABASE_PATH` (or only the previous scratch one), name a fresh
/// scratch database in the temp folder: a test never opens the NSH app's real data folder, and two engines
/// of one test run never share a file (the owner lock would refuse the second).
fn choose_database() -> Result<(), String> {
    if !test_environment() {
        return Ok(());
    }
    let named = std::env::var_os(DATABASE_PATH).filter(|value| !value.is_empty());
    let mut scratch = SCRATCH
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if named.is_some() && named != *scratch {
        return Ok(());
    }
    let root = std::env::temp_dir().join("ebirforms-door");
    if scratch.is_none() {
        forget_old_scratch(&root);
    }
    let folder = root.join(format!(
        "{}-{}",
        std::process::id(),
        SCRATCH_SEQUENCE.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("Cannot make a scratch database folder: {error}"))?;
    let path = folder.join("bir_data.db").into_os_string();
    // SAFETY: serialized by BOOT, at boot, before this thread's database opens.
    unsafe { std::env::set_var(DATABASE_PATH, &path) };
    *scratch = Some(path);
    Ok(())
}

/// Scratch folders of earlier runs older than a day go; best effort, and only ever inside `root`.
fn forget_old_scratch(root: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let day = std::time::Duration::from_secs(24 * 60 * 60);
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > day);
        if old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
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
    // SAFETY: serialized by BOOT; this thread has not opened its database yet.
    unsafe { std::env::set_var(DATABASE_PATH, &path) };
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
/// start the submission worker outside the boot.
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

/// Call a synchronous host function.
pub fn call(name: &str, args: Vec<JsonValue>) -> Result<JsonValue, String> {
    let module = booted()?;
    kind_of(&module, name, FunctionKind::Sync)?;
    permitted(name)?;
    module
        .call(name, &arguments(args))
        .map(|value| bir_app_api::bir_module::to_json(&value))
        .map_err(|error| error.message().to_owned())
}

/// Call an asynchronous host function.
pub async fn call_async(name: String, args: Vec<JsonValue>) -> Result<JsonValue, String> {
    let module = booted()?;
    kind_of(&module, &name, FunctionKind::Async)?;
    permitted(&name)?;
    let future = module
        .begin(&name, &arguments(args))
        .map_err(|error| error.message().to_owned())?;
    future
        .await
        .map(|value| bir_app_api::bir_module::to_json(&value))
        .map_err(|error| error.message().to_owned())
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
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::*;

    /// The dev environment of the plan, a database nobody else opens, and a home folder of its own.
    fn dev_env<R>(database: Option<&Path>, home: Option<&Path>, body: impl FnOnce() -> R) -> R {
        let database = database.map(|path| path.display().to_string());
        let home = home.map(|path| path.display().to_string());
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                ("EBIR_TEST_ENV", Some("1")),
                ("BIR_AUTH_BYPASS", Some("1")),
                ("BIR_SFTP_DRY_RUN", Some("1")),
                ("BIR_SFTP_LIVE", None),
                ("BIR_SFTP_HOST", None),
                ("BIR_DATABASE_PATH", database.as_deref()),
                (
                    "HOME",
                    home.as_deref().or(std::env::var("HOME").ok().as_deref()),
                ),
            ],
            body,
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
}
