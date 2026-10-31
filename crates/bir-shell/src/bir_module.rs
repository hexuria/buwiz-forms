//! The `bir` host module: the JavaScript-facing bridge into bir-core.
//!
//! Everything here crosses the boundary as `HostValue` — JSON-shaped records,
//! never Rust types. Functions are synchronous (main thread, no engine
//! re-entry); slow work (SFTP, exports) gets an `async_function` in a later
//! slice. The database opens lazily through `open_live_database` so
//! `BIR_DATABASE_PATH` keeps working for tests exactly like painted `bir`.

use std::cell::RefCell;

use bir_core::db::{self, Database};
use bir_core::forms::FORM_REGISTRY;
use bir_core::forms::queueable::QueueableForm as _;
use gpui_shell::{HostError, HostModule, HostObject, HostResult, HostValue};

use crate::auth_limiter;

thread_local! {
    static DATABASE: RefCell<Option<std::sync::Arc<std::sync::Mutex<Database>>>> =
        const { RefCell::new(None) };
    /// The handle holds an in-memory placeholder because [`reset_database`]
    /// let go of the live file; the next call reopens the file into it.
    static RELEASED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// [`stored_global_hotkey`] as last read or written: `None` until read.
    /// The tray asks on the UI thread, which must not wait on the database
    /// lock the submission worker may be holding.
    static GLOBAL_HOTKEY: RefCell<Option<Option<String>>> = const { RefCell::new(None) };
}

/// The shared handle behind [`with_database`], exposed for host functions
/// whose work must *release* the lock mid-call — the inbox poller re-locks
/// internally during an IMAP fetch, so `check_inbox_now` clones this Arc,
/// drops its guard, then polls.
///
/// The handle is never replaced afterwards (a database import reopens the
/// file *inside* it). [`open_database_at_launch`] starts the submission
/// worker on it once the account gate is open, so the worker sees the same
/// database as the page.
///
/// Open or reuse the process database. Does not check the account gate and
/// does not start the submission worker. Cloud-account settings use this
/// while the filing UI is still locked.
fn open_database_slot() -> Result<std::sync::Arc<std::sync::Mutex<Database>>, HostError> {
    DATABASE.with(|slot| {
        let mut slot = slot.borrow_mut();
        match slot.as_ref() {
            None => {
                let (database, _path, _recovered) =
                    db::open_live_database().map_err(|error| HostError::new(error.to_string()))?;
                let handle = std::sync::Arc::new(std::sync::Mutex::new(database));
                *slot = Some(handle);
            }
            Some(handle) if RELEASED.get() => {
                let (database, _path, _recovered) =
                    db::open_live_database().map_err(|error| HostError::new(error.to_string()))?;
                *handle
                    .lock()
                    .map_err(|_| HostError::new("database lock poisoned"))? = database;
                RELEASED.set(false);
                forget_global_hotkey();
            }
            Some(_) => {}
        }
        Ok(slot.as_ref().expect("database opened").clone())
    })
}

/// The shared handle behind [`with_database`]. Refuses while the Buwiz
/// account gate is locked. The first unlocked open is what
/// [`open_database_at_launch`] uses to start the submission worker.
pub(crate) fn database_handle() -> Result<std::sync::Arc<std::sync::Mutex<Database>>, HostError> {
    crate::account_gate::ensure_unlocked()?;
    open_database_slot()
}

static SUBMISSION_WORKER: std::sync::OnceLock<()> = std::sync::OnceLock::new();
static WORKER_THREAD: std::sync::Mutex<Option<std::thread::JoinHandle<()>>> =
    std::sync::Mutex::new(None);
static OWNER_LOCK_RELEASED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Start `bir_core::background_cron` — queued submissions, their retries,
/// confirmation-inbox polls and scheduled jobs — once per process, on its own
/// thread and Tokio runtime, exactly as painted `bir` and `bir-headless`
/// do. It shares `db`, the one owner-locked connection, rather than opening
/// a second. `BIR_SFTP_DRY_RUN` is read by the worker's transport, unchanged.
fn start_submission_worker(db: std::sync::Arc<std::sync::Mutex<Database>>) {
    if SUBMISSION_WORKER.set(()).is_err() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("bir-submission-worker".into())
        .spawn(move || match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime.block_on(bir_core::background_cron::start_cron_jobs(db)),
            Err(error) => {
                tracing::error!("submission worker not started: no Tokio runtime: {error}");
            }
        });
    match spawned {
        Ok(handle) => {
            if let Ok(mut slot) = WORKER_THREAD.lock() {
                *slot = Some(handle);
            }
        }
        Err(error) => tracing::error!("submission worker not started: {error}"),
    }
}

/// Stop the submission worker and release the owner lock.
///
/// Quit (the menu, Cmd/Ctrl+Q, `process.exit`) and an update install call
/// this. The worker finishes the tick it is on, drops its database handle,
/// and this thread checkpoints and closes. A second call does nothing. Safe
/// when the database was never opened.
///
/// An update install calls it before starting the new process, because the
/// owner lock is a process-wide flock and the replacement cannot open the
/// database until this process has dropped it.
pub(crate) fn shutdown_for_quit() {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    bir_core::background_cron::request_shutdown();
    let handle = WORKER_THREAD.lock().ok().and_then(|mut slot| slot.take());
    if let Some(handle) = handle {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = handle.join();
            let _ = tx.send(());
        });
        if rx.recv_timeout(std::time::Duration::from_secs(5)).is_err() {
            tracing::warn!("submission worker did not stop within 5s; quitting anyway");
        }
    }
    release_database();
}

fn release_database() {
    DATABASE.with(|slot| {
        let Some(handle) = slot.borrow_mut().take() else {
            OWNER_LOCK_RELEASED.store(true, std::sync::atomic::Ordering::SeqCst);
            return;
        };
        match std::sync::Arc::try_unwrap(handle) {
            Ok(mutex) => {
                let db = match mutex.into_inner() {
                    Ok(db) => db,
                    Err(poisoned) => poisoned.into_inner(),
                };
                if let Err(error) = db.close() {
                    tracing::warn!("database close failed: {error}");
                }
                // `close` drops the owner lock on both success and a failed
                // checkpoint: the fields were moved out, so the guard drops
                // when this function returns.
                OWNER_LOCK_RELEASED.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            Err(handle) => match handle.try_lock() {
                Ok(db) => {
                    if let Err(error) = db.checkpoint() {
                        tracing::warn!("database checkpoint failed: {error}");
                    }
                }
                Err(_) => tracing::warn!(
                    "database still in use at quit; the owner lock releases when the process exits"
                ),
            },
        }
    });
}

/// The stored global-show chord, if the database is already open and a key
/// is saved. Does not open the database: a missing handle means "not set".
/// Read from the database once, then from memory: [`set_global_hotkey`]
/// writes through, and a reopened database forgets it.
pub(crate) fn stored_global_hotkey() -> Option<String> {
    if let Some(known) = GLOBAL_HOTKEY.with(|cached| cached.borrow().clone()) {
        return known;
    }
    let handle = DATABASE.with(|slot| slot.borrow().clone())?;
    let stored = {
        let db = handle.lock().ok()?;
        db.get_setting("global_hotkey_key").ok()?
    };
    let combo = stored.filter(|value| !value.trim().is_empty());
    remember_global_hotkey(combo.clone());
    combo
}

fn remember_global_hotkey(combo: Option<String>) {
    GLOBAL_HOTKEY.with(|cached| *cached.borrow_mut() = Some(combo));
}

fn forget_global_hotkey() {
    GLOBAL_HOTKEY.with(|cached| *cached.borrow_mut() = None);
}

/// Whether [`shutdown_for_quit`] dropped the owner lock. An update refuses to
/// relaunch when this is still false: the new process would lose the race
/// for the encrypted database.
pub(crate) fn owner_lock_released() -> bool {
    OWNER_LOCK_RELEASED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Open the database when the shell launches; `host::run` calls this.
///
/// Also adopts the legacy log file and prunes to the user's retention, then
/// (in the shipping binary) re-checks that every six hours. Tests do not
/// start the sleeper thread: it would keep the test process alive.
pub(crate) fn open_database_at_launch() {
    if !crate::account_gate::is_unlocked() {
        return;
    }
    match open_database_slot() {
        Ok(handle) => {
            start_submission_worker(handle.clone());
            if let Ok(guard) = handle.lock() {
                let _ = bir_core::log_files::maintain(&bir_core::log_files::log_dir(), &guard);
            }
            #[cfg(not(test))]
            bir_core::log_files::spawn_retention(handle);
        }
        Err(error) => {
            tracing::warn!("database not opened at launch: {}", error.message());
        }
    }
}

/// Whether this process started the submission worker.
#[cfg(test)]
pub(crate) fn submission_worker_started() -> bool {
    SUBMISSION_WORKER.get().is_some()
}

#[cfg(test)]
thread_local! {
    static SUBMISSION_WORKER_WAKES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// How many times this thread's host calls woke the submission worker.
#[cfg(test)]
pub(crate) fn submission_worker_wakes() -> u32 {
    SUBMISSION_WORKER_WAKES.with(std::cell::Cell::get)
}

/// Tell the submission worker the queue changed, so a queued return goes out
/// now rather than on the worker's next one-minute tick (the painted views
/// and the agent host do the same after Queue and Cancel queue).
pub(crate) fn wake_submission_worker() {
    #[cfg(test)]
    SUBMISSION_WORKER_WAKES.with(|wakes| wakes.set(wakes.get() + 1));
    bir_core::background_cron::wake();
}

/// One connection for the process — the same owner-lock discipline as painted
/// `bir` (a second app instance cannot take it while we hold it).
pub(crate) fn with_database<R>(
    f: impl FnOnce(&Database) -> Result<R, HostError>,
) -> Result<R, HostError> {
    crate::account_gate::ensure_unlocked()?;
    with_account_database(f)
}

/// Database access for the Buwiz account itself: the server URL and nothing
/// that names a taxpayer. Allowed while the filing UI is locked.
pub(crate) fn with_account_database<R>(
    f: impl FnOnce(&Database) -> Result<R, HostError>,
) -> Result<R, HostError> {
    let handle = open_database_slot()?;
    let guard = handle
        .lock()
        .map_err(|_| HostError::new("database lock poisoned"))?;
    f(&guard)
}

/// Tests install a fresh ephemeral database in this thread's slot, so each
/// test (one thread each) runs against its own scratch data and never opens
/// the live database through `BIR_DATABASE_PATH`.
#[cfg(test)]
pub(crate) fn use_ephemeral_database() {
    DATABASE.with(|slot| {
        *slot.borrow_mut() = Some(std::sync::Arc::new(std::sync::Mutex::new(
            Database::open_ephemeral().expect("ephemeral database opens"),
        )));
    });
    RELEASED.set(false);
    forget_global_hotkey();
}

/// `serde_json::Value` → `HostValue`. bir-core serializes cleanly; the
/// boundary keeps JSON's six shapes exactly.
pub(crate) fn json(value: serde_json::Value) -> HostValue {
    match value {
        serde_json::Value::Null => HostValue::Null,
        serde_json::Value::Bool(v) => HostValue::Bool(v),
        serde_json::Value::Number(v) => HostValue::Number(v.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(v) => HostValue::Str(v),
        serde_json::Value::Array(v) => HostValue::Array(v.into_iter().map(json).collect()),
        serde_json::Value::Object(v) => HostValue::Object(
            v.into_iter()
                .map(|(key, value)| (key, json(value)))
                .collect(),
        ),
    }
}

pub(crate) fn serialize<T: serde::Serialize>(value: &T) -> Result<HostValue, HostError> {
    serde_json::to_value(value)
        .map(json)
        .map_err(|error| HostError::new(error.to_string()))
}

/// `HostValue` → `serde_json::Value`: the inverse of `json`, for functions
/// that take structured arguments (a whole profile record, a draft).
pub(crate) fn to_json(value: &HostValue) -> serde_json::Value {
    match value {
        HostValue::Null => serde_json::Value::Null,
        HostValue::Bool(v) => serde_json::Value::Bool(*v),
        // Script numbers arrive as f64, but typed drafts declare ints — an
        // integral double must round-trip as a JSON integer or serde rejects
        // `0.0` for `u16` fields.
        HostValue::Number(v) => {
            if v.fract() == 0.0 && *v >= i64::MIN as f64 && *v <= i64::MAX as f64 {
                serde_json::json!(*v as i64)
            } else {
                serde_json::json!(v)
            }
        }
        HostValue::Str(v) => serde_json::Value::String(v.clone()),
        HostValue::Array(v) => serde_json::Value::Array(v.iter().map(to_json).collect()),
        HostValue::Object(v) => serde_json::Value::Object(
            v.iter()
                .map(|(key, value)| (key.clone(), to_json(value)))
                .collect(),
        ),
    }
}

/// Hotkey, log retention, and the paths Settings shows. A database that is
/// not open yet answers the defaults; this must not fail the boot read.
fn computer_snapshot() -> (Option<String>, u32) {
    let Some(handle) = DATABASE.with(|slot| slot.borrow().clone()) else {
        return (None, bir_core::log_files::DEFAULT_RETENTION_DAYS);
    };
    let Ok(db) = handle.lock() else {
        return (None, bir_core::log_files::DEFAULT_RETENTION_DAYS);
    };
    let retention = bir_core::log_files::retention_days(&db);
    drop(db);
    (stored_global_hotkey(), retention)
}

/// Chords the Settings picker offers. The same combo language as
/// [`crate::hotkey::build_hotkey`]: explicit modifiers, not a bare key.
fn hotkey_presets() -> Vec<HostValue> {
    let (stored_prefix, label_prefix) = if cfg!(target_os = "macos") {
        ("control+alt", "Ctrl+Option")
    } else {
        ("super+shift", "Super+Shift")
    };
    ["E", "B", "H", "F12"]
        .into_iter()
        .map(|key| {
            HostValue::from(
                HostObject::new()
                    .field("value", format!("{stored_prefix}+{key}"))
                    .field("label", format!("{label_prefix}+{key}")),
            )
        })
        .collect()
}

fn system_info() -> HostResult {
    let (hotkey, retention) = computer_snapshot();
    let choices = bir_core::log_files::RETENTION_CHOICES
        .iter()
        .map(|days| HostValue::from(i64::from(*days)))
        .collect::<Vec<_>>();
    Ok(HostObject::new()
        .field("os", std::env::consts::OS)
        .field("arch", std::env::consts::ARCH)
        .field("app_version", env!("CARGO_PKG_VERSION"))
        .field(
            "database_path",
            db::app_database_path().display().to_string(),
        )
        .field(
            "data_dir",
            bir_core::platform::data_dir().display().to_string(),
        )
        .field(
            "logs_dir",
            bir_core::log_files::log_dir().display().to_string(),
        )
        .field(
            "global_hotkey",
            hotkey.map_or(HostValue::Null, HostValue::Str),
        )
        .field("hotkey_presets", HostValue::Array(hotkey_presets()))
        .field("log_retention_days", i64::from(retention))
        .field("log_retention_choices", HostValue::Array(choices))
        .field("runtime", "gpui-shell")
        .into())
}

/// Store the global show chord the tray already polls, or clear it.
/// An unknown key is refused and the stored chord is left as it was.
fn set_global_hotkey(arguments: &gpui_shell::HostArguments) -> HostResult {
    let combo = arguments.string(0)?.trim().to_string();
    if !combo.is_empty() && crate::hotkey::build_hotkey(&combo).is_none() {
        return Err(HostError::new(
            "That shortcut is not one this computer can register. Use a letter, digit, or F-key with modifiers, like super+shift+E.",
        ));
    }
    with_database(|db| {
        db.set_setting("global_hotkey_key", &combo)
            .map_err(|error| HostError::new(error.to_string()))
    })?;
    remember_global_hotkey(Some(combo).filter(|combo| !combo.is_empty()));
    Ok(HostValue::Null)
}

/// How many days of `ebirforms.*.log` to keep. Out of range is refused.
fn set_log_retention(arguments: &gpui_shell::HostArguments) -> HostResult {
    let days = arguments.integer(0)?;
    if !(i64::from(bir_core::log_files::MIN_RETENTION_DAYS)
        ..=i64::from(bir_core::log_files::MAX_RETENTION_DAYS))
        .contains(&days)
    {
        return Err(HostError::new(format!(
            "Log retention must be between {} and {} days.",
            bir_core::log_files::MIN_RETENTION_DAYS,
            bir_core::log_files::MAX_RETENTION_DAYS
        )));
    }
    with_database(|db| {
        db.set_setting(bir_core::log_files::RETENTION_SETTING, &days.to_string())
            .map(|_| HostValue::Null)
            .map_err(|error| HostError::new(error.to_string()))?;
        let _ = bir_core::log_files::maintain(&bir_core::log_files::log_dir(), db);
        Ok(HostValue::Null)
    })
}

fn update_status() -> HostResult {
    let status = crate::updater::current_status();
    let version = match status.version {
        Some(version) => HostValue::from(version),
        None => HostValue::Null,
    };
    let message = match status.message {
        Some(message) => HostValue::from(message),
        None => HostValue::Null,
    };
    Ok(HostObject::new()
        .field("phase", status.phase)
        .field("version", version)
        .field("message", message)
        .field("auto_download", status.auto_download)
        .field("current_version", status.current_version)
        .into())
}

fn install_update() -> HostResult {
    crate::updater::install_ready_update();
    Ok(HostValue::Null)
}

fn dismiss_update() -> HostResult {
    crate::updater::dismiss();
    Ok(HostValue::Null)
}

fn check_for_updates() -> HostResult {
    crate::updater::check_now();
    Ok(HostValue::Null)
}

fn set_auto_update_download(arguments: &gpui_shell::HostArguments) -> HostResult {
    let enabled = arguments.boolean(0)?;
    with_database(|db| {
        db.set_setting(
            crate::updater::AUTO_DOWNLOAD_SETTING,
            if enabled { "1" } else { "0" },
        )
        .map_err(|error| HostError::new(error.to_string()))?;
        crate::updater::set_auto_download(enabled);
        Ok(HostValue::Null)
    })
}

fn list_profiles() -> HostResult {
    with_database(|db| {
        let profiles = db
            .list_profiles()
            .map_err(|error| HostError::new(error.to_string()))?;
        let mut rows = Vec::with_capacity(profiles.len());
        for profile in &profiles {
            let tin = profile.tin.full();
            rows.push(
                HostObject::new()
                    .field("id", profile.id.unwrap_or_default())
                    .field("tin", tin)
                    .field("full_name", profile.full_name.as_str())
                    .field("rdo_code", profile.rdo_code.as_str())
                    .field("email", profile.email.as_str())
                    .field("taxpayer_type", format!("{:?}", profile.taxpayer_type))
                    .field("is_vat_registered", profile.is_vat_registered)
                    .field("is_archived", profile.is_archived)
                    .field("default_form_type", profile.default_form_type.as_str())
                    .field(
                        "applicable_forms",
                        profile
                            .applicable_forms()
                            .into_iter()
                            .map(HostValue::Str)
                            .collect::<Vec<_>>(),
                    ),
            );
        }
        Ok(HostValue::Array(
            rows.into_iter().map(HostValue::from).collect(),
        ))
    })
}

fn forms_catalog() -> HostResult {
    let mut rows = Vec::with_capacity(FORM_REGISTRY.len());
    for form in FORM_REGISTRY {
        rows.push(HostValue::from(
            HostObject::new()
                .field("code", form.code)
                .field("title", form.title)
                .field("category", form.category)
                .field("frequency", format!("{:?}", form.frequency))
                .field("requires_employees", form.requires_employees)
                .field("is_deprecated", form.is_deprecated),
        ));
    }
    Ok(HostValue::Array(rows))
}

/// The only settings the page may use through `get_setting`, `set_setting`
/// and `delete_setting`: its theme toggle's (`THEME_PREF_KEY`,
/// `app/shell/theme.js`). Every other setting is host state, such as the
/// access mode, the admin's TIN root, PIN hash and authenticator secret, and
/// the recovery key's hash; host code reads and writes those through
/// `Database` directly.
const PAGE_SETTING_KEYS: [&str; 1] = ["theme_preference"];

/// The key a page setting verb names, when it is in [`PAGE_SETTING_KEYS`].
/// Any other key is refused before the database is opened, with an error
/// that names the key and nothing stored under it.
fn page_setting_key(arguments: &gpui_shell::HostArguments) -> Result<String, HostError> {
    let key = arguments.string(0)?;
    if PAGE_SETTING_KEYS.contains(&key) {
        Ok(key.to_owned())
    } else {
        Err(HostError::new(format!(
            "setting `{key}` is not available to the page"
        )))
    }
}

fn get_setting(arguments: &gpui_shell::HostArguments) -> HostResult {
    let key = page_setting_key(arguments)?;
    with_database(|db| {
        db.get_setting(&key)
            .map(HostValue::from)
            .map_err(|error| HostError::new(error.to_string()))
    })
}

fn set_setting(arguments: &gpui_shell::HostArguments) -> HostResult {
    let key = page_setting_key(arguments)?;
    let value = arguments.string(1)?.to_owned();
    with_database(|db| {
        db.set_setting(&key, &value)
            .map(|_| HostValue::Null)
            .map_err(|error| HostError::new(error.to_string()))
    })
}

// ---------- S2: auth + profile management ----------

fn get_profile_by_tin(
    db: &Database,
    tin: &str,
) -> Result<bir_core::profile::TaxpayerProfile, HostError> {
    db.get_profile_by_tin(tin)
        .map_err(|error| HostError::new(error.to_string()))?
        .ok_or_else(|| HostError::new(format!("no taxpayer profile with TIN {tin}")))
}

/// A profile as the page receives it: the record without the credential
/// keys (removed, not sent as null), built by bir-core's
/// `TaxpayerProfile::record_without_credentials`, which keeps the values out
/// of the profile while the record is built.
///
/// A profile's five credentials (bir-core's `PROFILE_CREDENTIAL_KEYS`) are
/// host state: the page can neither read them nor set or clear them through
/// the profile verbs. Each keeps its own host writer instead: the inbox
/// verbs (`set_inbox_app_password`, the Google grant's persist and
/// disconnect) for the app password and the tokens; the authenticator verbs
/// (`set_taxpayer_totp`, `set_profile_totp`, `set_admin_totp`) for a TOTP
/// secret; and the PIN verbs (`set_taxpayer_pin`, `set_profile_pin`,
/// `set_admin_pin`, `setup_helpdesk`, `change_to_helpdesk`) for a PIN. The
/// authenticator and PIN verbs store theirs on the taxpayer (the help desk
/// admin's in settings), never on a profile. A TOTP secret or PIN hash still
/// on a profile row moves onto its taxpayer at the next launch when the
/// taxpayer has none (bir-core's `migrate_v22_taxpayer_credentials`), so one
/// the page could set would become the taxpayer's.
fn redacted_profile(mut profile: bir_core::profile::TaxpayerProfile) -> HostResult {
    profile
        .record_without_credentials()
        .map(json)
        .map_err(db_error)
}

/// `save_profile` never stores a client's credentials, as
/// [`keep_stored_trigger_flag`] never stores a client's trigger fields: all
/// five are taken from the stored row the save writes, or cleared when there
/// is none. So a page round trip of a redacted profile keeps a stored
/// password or PIN hash, and no TOTP secret or PIN hash from the page
/// reaches a row the next launch would move onto the taxpayer. The row is
/// found the way [`Database::save_profile`] finds it: the one with the
/// client's `id` (a TIN change re-keys that row), else the one with the
/// client's TIN.
fn keep_stored_credentials(
    db: &Database,
    profile: &mut bir_core::profile::TaxpayerProfile,
) -> Result<(), HostError> {
    let by_tin = db
        .get_profile_by_tin(&profile.tin.full())
        .map_err(db_error)?;
    let mut stored = match profile.id {
        // bir-core looks rows up by TIN only, so the list is read just when
        // the id is not the row with this TIN (a TIN change).
        Some(id) if !by_tin.as_ref().is_some_and(|row| row.id == Some(id)) => db
            .list_profiles()
            .map_err(db_error)?
            .into_iter()
            .find(|row| row.id == Some(id))
            .or(by_tin),
        _ => by_tin,
    };
    let credentials = stored
        .as_mut()
        .map(bir_core::profile::TaxpayerProfile::take_credentials)
        .unwrap_or_default();
    profile.put_credentials(credentials);
    Ok(())
}

/// The stored profile for `tin`, without its credentials.
fn profile_detail(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| redacted_profile(get_profile_by_tin(db, tin)?))
}

/// Save the page's profile. Its credentials are settled from the stored row
/// ([`keep_stored_credentials`]); the answer is the saved profile without
/// them.
fn save_profile(arguments: &gpui_shell::HostArguments) -> HostResult {
    let mut profile: bir_core::profile::TaxpayerProfile =
        serde_json::from_value(to_json(arguments.value(0)?))
            .map_err(|error| HostError::new(format!("profile payload: {error}")))?;
    with_database(|db| {
        keep_stored_credentials(db, &mut profile)?;
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))
            .and_then(redacted_profile)
    })
}

fn set_profile_archived(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let archived = arguments.boolean(1)?;
    with_database(|db| {
        if archived {
            guard_last_admin_branch(db, &tin, "archived", |profile| profile.is_archived)?;
        }
        let mut profile = get_profile_by_tin(db, &tin)?;
        profile.is_archived = archived;
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))
            .and_then(redacted_profile)
    })
}

fn delete_profile(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| {
        guard_last_admin_branch(db, tin, "deleted", |_| false)?;
        db.delete_profile(tin)
            .map(|_| HostValue::Null)
            .map_err(|error| HostError::new(error.to_string()))
    })
}

// ---------- sign-in: one lock per taxpayer, admin by access mode ----------
//
// A PIN and an authenticator belong to the taxpayer (9-digit TIN root) and
// are stored once, on the `taxpayers` row; every branch shares them. The
// admin account depends on the access mode chosen at first launch:
// - `solo`: the admin IS the owner's taxpayer (`admin_tin_root`), so the
//   admin PIN/authenticator are that taxpayer's.
// - `helpdesk`: the admin is the reserved root `000000000`, not a taxpayer;
//   its PIN/authenticator are the `app_lock_pin_hash` / `app_totp_secret`
//   settings, and it must always keep at least one of them.
// Recovery ladder: authenticator -> recovery key -> Mac login.

const ACCESS_MODE: &str = "access_mode";
const ADMIN_TIN_ROOT: &str = "admin_tin_root";
const ADMIN_PIN_HASH: &str = "app_lock_pin_hash";
const ADMIN_TOTP_SECRET: &str = "app_totp_secret";
const RECOVERY_KEY_HASH: &str = "admin_recovery_key_hash";

const WRONG_CODE: &str =
    "That code does not match. Type the 6-digit code your authenticator app shows now.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum AccessMode {
    /// The owner's own taxpayer is the admin.
    Solo(String),
    Helpdesk,
}

fn db_error(error: impl std::fmt::Display) -> HostError {
    HostError::new(error.to_string())
}

fn setting(db: &Database, key: &str) -> Result<Option<String>, HostError> {
    db.get_setting(key).map_err(db_error)
}

/// The chosen access mode; `None` on a fresh install not yet set up.
fn access_mode(db: &Database) -> Result<Option<AccessMode>, HostError> {
    Ok(match setting(db, ACCESS_MODE)?.as_deref() {
        Some("solo") => setting(db, ADMIN_TIN_ROOT)?.map(AccessMode::Solo),
        Some("helpdesk") => Some(AccessMode::Helpdesk),
        _ => None,
    })
}

fn require_access_mode(db: &Database) -> Result<AccessMode, HostError> {
    access_mode(db)?.ok_or_else(|| HostError::new("This app is not set up yet."))
}

/// The TIN root of the solo owner, whose taxpayer must never disappear.
fn solo_admin_root(db: &Database) -> Result<Option<String>, HostError> {
    Ok(match access_mode(db)? {
        Some(AccessMode::Solo(root)) => Some(root),
        _ => None,
    })
}

fn tin_root_argument(raw: &str) -> Result<String, HostError> {
    bir_core::tax_profile::parse_tin_root(raw).map_err(db_error)
}

/// A PIN argument: `null` clears, otherwise exactly 4 digits.
fn pin_argument(value: &HostValue) -> Result<Option<String>, HostError> {
    match value {
        HostValue::Null => Ok(None),
        HostValue::Str(pin) if pin.len() == 4 && pin.bytes().all(|b| b.is_ascii_digit()) => {
            Ok(Some(pin.clone()))
        }
        _ => Err(HostError::new("A PIN is 4 digits.")),
    }
}

/// An authenticator argument pair: `null` clears; a secret needs a current
/// code from the app, checked against the new secret, so a mistyped
/// enrolment cannot lock anyone out.
fn totp_argument(
    arguments: &gpui_shell::HostArguments,
    secret_index: usize,
) -> Result<Option<String>, HostError> {
    let secret = arguments.value(secret_index)?.as_str().map(str::to_owned);
    let token = arguments
        .get(secret_index + 1)
        .and_then(HostValue::as_str)
        .unwrap_or_default();
    if let Some(secret) = &secret
        && !bir_core::crypto::validate_totp(secret, token)
    {
        return Err(HostError::new(WRONG_CODE));
    }
    Ok(secret)
}

fn taxpayer_credentials(
    db: &Database,
    tin_root: &str,
) -> Result<bir_core::tax_profile::TaxpayerCredentials, HostError> {
    db.taxpayer_credentials(tin_root).map_err(db_error)
}

/// The admin's PIN hash and authenticator secret under `mode`.
fn admin_credentials(
    db: &Database,
    mode: &AccessMode,
) -> Result<bir_core::tax_profile::TaxpayerCredentials, HostError> {
    match mode {
        AccessMode::Solo(root) => taxpayer_credentials(db, root),
        AccessMode::Helpdesk => Ok(bir_core::tax_profile::TaxpayerCredentials {
            pin_hash: setting(db, ADMIN_PIN_HASH)?,
            totp_secret: setting(db, ADMIN_TOTP_SECRET)?,
        }),
    }
}

fn pin_matches(stored: Option<&str>, pin: &str) -> bool {
    stored.is_some_and(|stored| stored == bir_core::crypto::hash_pin(pin))
}

fn totp_matches(secret: Option<&str>, token: &str) -> bool {
    secret.is_some_and(|secret| bir_core::crypto::validate_totp(secret, token))
}

fn generate_totp_secret(arguments: &gpui_shell::HostArguments) -> HostResult {
    let account = arguments.string(0)?;
    let (secret, qr_path) = bir_core::crypto::generate_totp_secret(account);
    Ok(HostObject::new()
        .field("secret", secret)
        .field("qr_path", qr_path.display().to_string())
        .into())
}

fn delete_setting(arguments: &gpui_shell::HostArguments) -> HostResult {
    let key = page_setting_key(arguments)?;
    with_database(|db| {
        db.delete_setting(&key)
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

/// What the lock screen and first-launch setup read once per render.
fn auth_state() -> HostResult {
    with_database(|db| {
        let mode = access_mode(db)?;
        let credentials = match &mode {
            Some(mode) => admin_credentials(db, mode)?,
            None => Default::default(),
        };
        let (mode_name, admin_root) = match &mode {
            Some(AccessMode::Solo(root)) => {
                (HostValue::from("solo"), HostValue::from(root.as_str()))
            }
            Some(AccessMode::Helpdesk) => (
                HostValue::from("helpdesk"),
                HostValue::from(bir_core::tax_profile::RESERVED_ADMIN_TIN_ROOT),
            ),
            None => (HostValue::Null, HostValue::Null),
        };
        Ok(HostObject::new()
            .field("mode", mode_name)
            .field("admin_root", admin_root)
            .field("has_admin_pin", credentials.pin_hash.is_some())
            .field("has_admin_totp", credentials.totp_secret.is_some())
            .field(
                "has_recovery_key",
                setting(db, RECOVERY_KEY_HASH)?.is_some(),
            )
            .field("mac_login_available", mac_login_available())
            .into())
    })
}

fn taxpayer_auth(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    with_database(|db| {
        let credentials = taxpayer_credentials(db, &tin_root)?;
        Ok(HostObject::new()
            .field("has_pin", credentials.pin_hash.is_some())
            .field("has_totp", credentials.totp_secret.is_some())
            .into())
    })
}

/// Set (`pin`) or clear (`null`) a taxpayer's PIN.
fn set_taxpayer_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let pin = pin_argument(arguments.value(1)?)?;
    with_database(|db| write_taxpayer_pin(db, &tin_root, pin.as_deref()))
}

fn write_taxpayer_pin(db: &Database, tin_root: &str, pin: Option<&str>) -> HostResult {
    let hash = pin.map(bir_core::crypto::hash_pin);
    db.set_taxpayer_pin_hash(tin_root, hash.as_deref())
        .map(|_| HostValue::Null)
        .map_err(db_error)
}

/// Set (with a current code) or clear (`null`) a taxpayer's authenticator.
fn set_taxpayer_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let secret = totp_argument(arguments, 1)?;
    with_database(|db| {
        db.set_taxpayer_totp_secret(&tin_root, secret.as_deref())
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

fn verify_taxpayer_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let pin = arguments.string(1)?;
    let ok = auth_limiter::guarded(&auth_limiter::taxpayer(&tin_root), || {
        with_database(|db| {
            let credentials = taxpayer_credentials(db, &tin_root)?;
            Ok(pin_matches(credentials.pin_hash.as_deref(), pin))
        })
    })?;
    Ok(HostValue::Bool(ok))
}

fn verify_taxpayer_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let token = arguments.string(1)?;
    let ok = auth_limiter::guarded(&auth_limiter::taxpayer(&tin_root), || {
        with_database(|db| {
            let credentials = taxpayer_credentials(db, &tin_root)?;
            Ok(totp_matches(credentials.totp_secret.as_deref(), token))
        })
    })?;
    Ok(HostValue::Bool(ok))
}

/// Legacy per-branch entry point: a branch TIN's PIN is its taxpayer's.
fn set_profile_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let pin = pin_argument(arguments.value(1)?)?;
    with_database(|db| {
        let root = get_profile_by_tin(db, &tin)?.tin.full()[..9].to_string();
        write_taxpayer_pin(db, &root, pin.as_deref())
    })
}

/// Legacy per-branch entry point: a branch TIN's authenticator is its
/// taxpayer's. Setting needs a current code; `null` clears.
fn set_profile_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let secret = totp_argument(arguments, 1)?;
    with_database(|db| {
        let root = get_profile_by_tin(db, &tin)?.tin.full()[..9].to_string();
        db.set_taxpayer_totp_secret(&root, secret.as_deref())
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

fn verify_admin_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin = arguments.string(0)?;
    let ok = auth_limiter::guarded(auth_limiter::ADMIN, || {
        with_database(|db| {
            Ok(match access_mode(db)? {
                Some(mode) => pin_matches(admin_credentials(db, &mode)?.pin_hash.as_deref(), pin),
                None => false,
            })
        })
    })?;
    Ok(HostValue::Bool(ok))
}

fn verify_admin_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let token = arguments.string(0)?;
    let ok = auth_limiter::guarded(auth_limiter::ADMIN, || {
        with_database(|db| {
            Ok(match access_mode(db)? {
                Some(mode) => {
                    totp_matches(admin_credentials(db, &mode)?.totp_secret.as_deref(), token)
                }
                None => false,
            })
        })
    })?;
    Ok(HostValue::Bool(ok))
}

const KEEP_ONE_WAY_IN: &str = "The admin must keep at least one way to sign in. Set up the other one (PIN or authenticator) before removing this one.";

/// Set or clear the admin PIN. Solo: the owner taxpayer's PIN (clearing is
/// allowed). Help desk: the settings PIN, refused to clear without an
/// authenticator. Only the hash is stored; the script never sees it.
fn set_admin_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin = pin_argument(arguments.value(0)?)?;
    with_database(|db| match require_access_mode(db)? {
        AccessMode::Solo(root) => write_taxpayer_pin(db, &root, pin.as_deref()),
        AccessMode::Helpdesk => match pin {
            Some(pin) => db.set_setting(ADMIN_PIN_HASH, &bir_core::crypto::hash_pin(&pin)),
            None => {
                if setting(db, ADMIN_TOTP_SECRET)?.is_none() {
                    return Err(HostError::new(KEEP_ONE_WAY_IN));
                }
                db.delete_setting(ADMIN_PIN_HASH)
            }
        }
        .map(|_| HostValue::Null)
        .map_err(db_error),
    })
}

/// Set (with a current code) or clear the admin authenticator, mode-aware
/// like [`set_admin_pin`].
fn set_admin_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let secret = totp_argument(arguments, 0)?;
    with_database(|db| match require_access_mode(db)? {
        AccessMode::Solo(root) => db
            .set_taxpayer_totp_secret(&root, secret.as_deref())
            .map(|_| HostValue::Null)
            .map_err(db_error),
        AccessMode::Helpdesk => match secret {
            Some(secret) => db.set_setting(ADMIN_TOTP_SECRET, &secret),
            None => {
                if setting(db, ADMIN_PIN_HASH)?.is_none() {
                    return Err(HostError::new(KEEP_ONE_WAY_IN));
                }
                db.delete_setting(ADMIN_TOTP_SECRET)
            }
        }
        .map(|_| HostValue::Null)
        .map_err(db_error),
    })
}

const ALREADY_SET_UP: &str = "This app is already set up.";

/// First launch, "Just me or my business": the taxpayer the script just
/// created is the owner and the admin.
fn setup_solo(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    with_database(|db| {
        if setting(db, ACCESS_MODE)?.is_some() {
            return Err(HostError::new(ALREADY_SET_UP));
        }
        bir_core::tax_profile::reject_reserved_tin_root(&root).map_err(db_error)?;
        if db.get_taxpayer(&root).map_err(db_error)?.is_none() {
            return Err(HostError::new(format!("No taxpayer with TIN {root}")));
        }
        db.set_setting(ADMIN_TIN_ROOT, &root).map_err(db_error)?;
        db.set_setting(ACCESS_MODE, "solo").map_err(db_error)?;
        Ok(HostValue::Null)
    })
}

/// First launch, "I help other taxpayers": admin PIN plus a recovery key.
fn setup_helpdesk(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin =
        pin_argument(arguments.value(0)?)?.ok_or_else(|| HostError::new("A PIN is 4 digits."))?;
    with_database(|db| {
        if setting(db, ACCESS_MODE)?.is_some() {
            return Err(HostError::new(ALREADY_SET_UP));
        }
        db.set_setting(ADMIN_PIN_HASH, &bir_core::crypto::hash_pin(&pin))
            .map_err(db_error)?;
        db.set_setting(ACCESS_MODE, "helpdesk").map_err(db_error)?;
        db.delete_setting(ADMIN_TIN_ROOT).map_err(db_error)?;
        store_new_recovery_key(db)
    })
}

// ---------- change mode / owner (admin only, after first launch) ----------
//
// Settings › Computer setup. Each change re-checks the current admin's PIN
// or authenticator code here, never trusting the script, refuses a change
// that would leave nobody able to sign in as admin, and replaces the
// recovery key so the previous admin's key stops working.

const NOT_THE_ADMIN: &str =
    "That PIN or code is not the admin's. Type the current admin PIN or authenticator code.";

/// The current admin's PIN (`pin`) or authenticator code (`code`), checked
/// against the stored credentials of `mode`. Wrong tries count against the
/// admin's sign-in backoff, so this is no way around it.
fn confirm_admin(
    db: &Database,
    mode: &AccessMode,
    pin: Option<&str>,
    code: Option<&str>,
) -> Result<(), HostError> {
    let matched = auth_limiter::guarded(auth_limiter::ADMIN, || {
        let credentials = admin_credentials(db, mode)?;
        let by_pin = pin.is_some_and(|pin| pin_matches(credentials.pin_hash.as_deref(), pin));
        let by_code =
            code.is_some_and(|code| totp_matches(credentials.totp_secret.as_deref(), code));
        Ok(by_pin || by_code)
    })?;
    if matched {
        Ok(())
    } else {
        Err(HostError::new(NOT_THE_ADMIN))
    }
}

/// `null` or a non-empty string.
fn optional_text(arguments: &gpui_shell::HostArguments, index: usize) -> Option<String> {
    arguments
        .get(index)
        .and_then(HostValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// A taxpayer that can become the owner: exists, not the reserved root,
/// has a branch that is not archived, and has a PIN or authenticator so
/// someone can sign in as admin afterwards.
fn check_new_owner(db: &Database, root: &str) -> Result<(), HostError> {
    bir_core::tax_profile::reject_reserved_tin_root(root).map_err(db_error)?;
    let Some(taxpayer) = db.get_taxpayer(root).map_err(db_error)? else {
        return Err(HostError::new(format!("No taxpayer with TIN {root}")));
    };
    let mut active = false;
    for branch in &taxpayer.branches {
        if let Some(profile) = db.get_profile_by_tin(&branch.tin).map_err(db_error)?
            && !profile.is_archived
        {
            active = true;
            break;
        }
    }
    if !active {
        return Err(HostError::new(
            "This taxpayer is archived. Unarchive it before making it the owner.",
        ));
    }
    let credentials = taxpayer_credentials(db, root)?;
    if credentials.pin_hash.is_none() && credentials.totp_secret.is_none() {
        return Err(HostError::new(
            "This taxpayer has no PIN or authenticator yet. Set one for them first, so the new owner can sign in as admin.",
        ));
    }
    Ok(())
}

/// Make `tin_root` the owner (solo mode), from solo or from help desk.
/// Arguments: `tin_root`, then the current admin's `pin` and `code` (either
/// may be `null`). Help desk's admin PIN and authenticator are removed.
/// Answers a new recovery key.
fn change_owner(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    let pin = optional_text(arguments, 1);
    let code = optional_text(arguments, 2);
    with_database(|db| {
        let mode = require_access_mode(db)?;
        confirm_admin(db, &mode, pin.as_deref(), code.as_deref())?;
        if mode == AccessMode::Solo(root.clone()) {
            return Err(HostError::new("This taxpayer is already the owner."));
        }
        check_new_owner(db, &root)?;
        // Owner first, then the mode: a failure in between leaves a working
        // setup either way.
        db.set_setting(ADMIN_TIN_ROOT, &root).map_err(db_error)?;
        db.set_setting(ACCESS_MODE, "solo").map_err(db_error)?;
        db.delete_setting(ADMIN_PIN_HASH).map_err(db_error)?;
        db.delete_setting(ADMIN_TOTP_SECRET).map_err(db_error)?;
        store_new_recovery_key(db)
    })
}

/// Switch from solo to help desk. Arguments: the current admin's `pin` and
/// `code` (either may be `null`), then the new admin PIN. The former owner
/// stays an ordinary taxpayer with their own PIN. Answers a new recovery
/// key.
fn change_to_helpdesk(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin = optional_text(arguments, 0);
    let code = optional_text(arguments, 1);
    let new_pin =
        pin_argument(arguments.value(2)?)?.ok_or_else(|| HostError::new("A PIN is 4 digits."))?;
    with_database(|db| {
        let mode = require_access_mode(db)?;
        if mode == AccessMode::Helpdesk {
            return Err(HostError::new(
                "This computer is already set up for a help desk.",
            ));
        }
        confirm_admin(db, &mode, pin.as_deref(), code.as_deref())?;
        // The admin PIN first, then the mode: a failure in between leaves a
        // working solo setup.
        db.set_setting(ADMIN_PIN_HASH, &bir_core::crypto::hash_pin(&new_pin))
            .map_err(db_error)?;
        db.delete_setting(ADMIN_TOTP_SECRET).map_err(db_error)?;
        db.set_setting(ACCESS_MODE, "helpdesk").map_err(db_error)?;
        db.delete_setting(ADMIN_TIN_ROOT).map_err(db_error)?;
        store_new_recovery_key(db)
    })
}

/// A fresh recovery key replaces any earlier one; only its hash is kept.
fn store_new_recovery_key(db: &Database) -> HostResult {
    let key = bir_core::crypto::generate_recovery_key();
    db.set_setting(
        RECOVERY_KEY_HASH,
        &bir_core::crypto::hash_recovery_key(&key),
    )
    .map_err(db_error)?;
    Ok(HostObject::new().field("recovery_key", key).into())
}

fn create_recovery_key() -> HostResult {
    with_database(|db| {
        require_access_mode(db)?;
        store_new_recovery_key(db)
    })
}

/// Single use: a match answers true and forgets the key. Case, dashes and
/// spaces do not matter.
fn use_recovery_key(arguments: &gpui_shell::HostArguments) -> HostResult {
    let typed = bir_core::crypto::hash_recovery_key(arguments.string(0)?);
    let matches = auth_limiter::guarded(auth_limiter::RECOVERY_KEY, || {
        with_database(|db| {
            let matches = setting(db, RECOVERY_KEY_HASH)?.is_some_and(|stored| stored == typed);
            if matches {
                db.delete_setting(RECOVERY_KEY_HASH).map_err(db_error)?;
            }
            Ok(matches)
        })
    })?;
    Ok(HostValue::Bool(matches))
}

fn mac_login_available() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

/// Ask the operating system to confirm the signed-in user (Touch ID or the
/// account password on macOS, Windows Hello elsewhere). The prompt runs on
/// its own thread, so the UI thread never waits; other platforms answer
/// false.
fn mac_login(
    arguments: &gpui_shell::HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let reason = arguments
        .get(0)
        .and_then(HostValue::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("sign in as the admin")
        .to_owned();
    let (sender, receiver) = futures::channel::oneshot::channel::<bool>();
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    std::thread::Builder::new()
        .name("mac-login".into())
        .spawn(move || {
            let _ = sender.send(os_authenticate(&reason));
        })
        .map_err(db_error)?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = reason;
        let _ = sender.send(false);
    }
    Ok(async move { Ok(HostValue::Bool(receiver.await.unwrap_or(false))) })
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn os_authenticate(reason: &str) -> bool {
    use robius_authentication::{
        AndroidText, BiometricStrength, Context, PolicyBuilder, Text, WindowsText,
    };
    let Some(policy) = PolicyBuilder::new()
        .biometrics(Some(BiometricStrength::Strong))
        .password(true)
        .watch(true)
        .build()
    else {
        tracing::warn!("OS sign-in policy unavailable");
        return false;
    };
    let text = Text {
        android: AndroidText {
            title: "Admin sign-in",
            subtitle: None,
            description: None,
        },
        apple: reason,
        windows: WindowsText::new_truncated("Admin sign-in", reason),
    };
    futures::executor::block_on(Context::new(()).authenticate(text, &policy)).is_ok()
}

/// Refuse to archive or delete the solo owner (the admin) or the reserved
/// help desk root.
fn guard_admin_taxpayer(db: &Database, root: &str, action: &str) -> Result<(), HostError> {
    if root == bir_core::tax_profile::RESERVED_ADMIN_TIN_ROOT
        || solo_admin_root(db)?.as_deref() == Some(root)
    {
        return Err(HostError::new(format!(
            "This taxpayer is the admin account and cannot be {action}."
        )));
    }
    Ok(())
}

/// Archiving or deleting one branch must not take the solo owner's last
/// usable branch with it: refuse when every other branch of the admin
/// taxpayer is already `gone`.
fn guard_last_admin_branch(
    db: &Database,
    tin: &str,
    action: &str,
    gone: impl Fn(&bir_core::profile::TaxpayerProfile) -> bool,
) -> Result<(), HostError> {
    let Some(root) = solo_admin_root(db)? else {
        return Ok(());
    };
    if tin.len() < 9 || tin[..9] != root {
        return Ok(());
    }
    let Some(taxpayer) = db.get_taxpayer(&root).map_err(db_error)? else {
        return Ok(());
    };
    for branch in taxpayer.branches.iter().filter(|branch| branch.tin != tin) {
        if let Some(profile) = db.get_profile_by_tin(&branch.tin).map_err(db_error)?
            && !gone(&profile)
        {
            return Ok(());
        }
    }
    Err(HostError::new(format!(
        "This is the admin taxpayer's last branch and cannot be {action}."
    )))
}

fn set_taxpayer_archived(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    let archived = arguments.boolean(1)?;
    with_database(|db| {
        if archived {
            guard_admin_taxpayer(db, &root, "archived")?;
        }
        db.set_taxpayer_archived(&root, archived)
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

fn delete_taxpayer(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    with_database(|db| {
        guard_admin_taxpayer(db, &root, "deleted")?;
        db.delete_taxpayer(&root)
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

// ---------- S3: form drafts ----------

fn list_drafts(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let year = arguments.integer(1)? as u16;
    with_database(|db| {
        let rows = db
            .list_draft_summaries(&tin, year)
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Array(
            rows.iter()
                .map(|row| {
                    HostValue::from(
                        HostObject::new()
                            .field("id", row.id)
                            .field("tin", row.tin.as_str())
                            .field("form_code", row.form_code.as_str())
                            .field("taxable_year", row.taxable_year as i64)
                            .field(
                                "quarter",
                                row.quarter.map_or(HostValue::Null, HostValue::from),
                            )
                            .field("month", row.month.map_or(HostValue::Null, HostValue::from))
                            .field("status", format!("{:?}", row.status))
                            .field("updated_at", row.updated_at.as_str()),
                    )
                })
                .collect(),
        ))
    })
}

fn filing_status(name: &str) -> Result<bir_core::forms::FilingStatus, HostError> {
    use bir_core::forms::FilingStatus::*;
    match name {
        "Draft" => Ok(Draft),
        "Queued" => Ok(Queued),
        "Submitted" | "Filed" => Ok(Submitted),
        "Confirmed" => Ok(Confirmed),
        "Paid" => Ok(Paid),
        other => Err(HostError::new(format!("unknown filing status {other:?}"))),
    }
}

/// `quarter` argument is `Option<u8>`: pass null for annual/open-ended slots
/// and 0 for the first open-ended period (the DB auto-numbers those).
fn optional_quarter(
    arguments: &gpui_shell::HostArguments,
    index: usize,
) -> Result<Option<u8>, HostError> {
    match arguments.get(index) {
        Some(HostValue::Null) | None => Ok(None),
        Some(HostValue::Number(q)) => Ok(Some(*q as u8)),
        Some(other) => Err(HostError::new(format!(
            "quarter must be a number or null, got {}",
            other.describe()
        ))),
    }
}

fn get_form_draft(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    with_database(|db| {
        let draft: Option<serde_json::Value> = match form_code.as_str() {
            "2551Q" => {
                let q = quarter.unwrap_or(1);
                db.get_2551q_draft(&tin, year, q)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .map(|d| {
                        serde_json::to_value(&d).map_err(|error| HostError::new(error.to_string()))
                    })
                    .transpose()?
            }
            "1601C" => {
                let month = quarter.unwrap_or(1);
                db.get_1601c_draft(&tin, year, month)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .map(|d| {
                        serde_json::to_value(&d).map_err(|error| HostError::new(error.to_string()))
                    })
                    .transpose()?
            }
            _ => db
                .get_form_draft(&tin, &form_code, year, quarter)
                .map_err(|error| HostError::new(error.to_string()))?,
        };
        match draft {
            Some(value) => serialize(&value),
            None => Ok(HostValue::Null),
        }
    })
}

/// Fields the script reports merge over a profile-derived draft before the
/// typed save — the same overlay shape the painted app uses, so the draft
/// keeps every engine-required field even when the UI edits a subset.
fn overlay_fields<T: serde::Serialize + serde::de::DeserializeOwned>(
    base: T,
    data: &serde_json::Value,
) -> Result<T, HostError> {
    let mut merged =
        serde_json::to_value(&base).map_err(|error| HostError::new(error.to_string()))?;
    if let (Some(target), Some(source)) = (merged.as_object_mut(), data.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    serde_json::from_value(merged).map_err(|error| HostError::new(error.to_string()))
}

/// A generic save never stores a client's `trigger_date_model`,
/// `trigger_date_entered` or `trigger_parts_typed` (D8c, P5-7). The row it
/// checks is the one the save writes
/// ([`bir_core::db::Database::form_draft_for_save`]: a new open-ended
/// entry has none); that row's three fields are kept when it is the same
/// draft (`created_at`) and holds exactly these trigger fields, otherwise
/// all three are removed — this path creates no marker itself.
fn keep_stored_trigger_flag(
    db: &Database,
    tin: &str,
    form_code: &str,
    year: u16,
    quarter: Option<u8>,
    data: &mut serde_json::Value,
) -> Result<(), HostError> {
    let Some(object) = data.as_object_mut() else {
        return Ok(());
    };
    let fields = bir_core::forms::event_trigger::trigger_paths(form_code);
    let stored: Option<serde_json::Value> = if fields.is_empty() {
        None
    } else {
        db.form_draft_for_save(tin, form_code, year, quarter)
            .map_err(|error| HostError::new(error.to_string()))?
    };
    let same = stored.filter(|stored| {
        stored.get("created_at").is_some()
            && stored.get("created_at") == object.get("created_at")
            && fields
                .iter()
                .all(|field| stored.get(*field) == object.get(*field))
    });
    // The marker is host state like the typed record beside it (decision
    // P5-7): the stored row's is copied, a missing stored marker removes
    // the client's — this save is not a creator and never mints one.
    for field in [
        "trigger_date_model",
        "trigger_date_entered",
        "trigger_parts_typed",
    ] {
        match same.as_ref().and_then(|stored| stored.get(field)) {
            Some(value) => object.insert(field.into(), value.clone()),
            None => object.remove(field),
        };
    }
    Ok(())
}

fn save_form_draft(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    let status = filing_status(arguments.string(4)?)?;
    let mut data = to_json(arguments.value(5)?);
    with_database(|db| {
        // The engine requires dedicated typed persistence for these two forms;
        // route them inside the host so the script keeps one uniform call.
        match form_code.as_str() {
            "2551Q" => {
                let profile = db
                    .get_profile(&tin)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new(format!("no profile for TIN {tin}")))?;
                let q = quarter.ok_or_else(|| {
                    HostError::new("2551Q drafts are quarterly: pass quarter 1-4")
                })?;
                let base = bir_core::forms::Form2551QDraft::new_from_profile(&profile, year, q);
                let draft = overlay_fields(base, &data)?;
                db.save_2551q_draft(&draft)
                    .map(|id| HostValue::Number(id as f64))
                    .map_err(|error| HostError::new(error.to_string()))
            }
            "1601C" => {
                let profile = db
                    .get_profile(&tin)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new(format!("no profile for TIN {tin}")))?;
                let month = quarter.ok_or_else(|| {
                    HostError::new("1601C drafts are monthly: pass the month in the quarter slot")
                })?;
                let base = bir_core::forms::Form1601CDraft::new_from_profile(&profile, year, month);
                let draft = overlay_fields(base, &data)?;
                db.save_1601c_draft(&draft)
                    .map(|id| HostValue::Number(id as f64))
                    .map_err(|error| HostError::new(error.to_string()))
            }
            _ => {
                keep_stored_trigger_flag(db, &tin, &form_code, year, quarter, &mut data)?;
                db.save_form_draft(&tin, &form_code, year, quarter, &status, &data)
                    .map(|id| HostValue::Number(id as f64))
                    .map_err(|error| HostError::new(error.to_string()))
            }
        }
    })
}

/// Every queued submission across profiles — the queue page's feed.
fn list_queued() -> HostResult {
    with_database(|db| {
        let summaries = db
            .list_all_queued_submissions()
            .map_err(|error| HostError::new(error.to_string()))?;
        let items: Vec<serde_json::Value> = summaries
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "tin": s.tin,
                    "form_code": s.form_code,
                    "taxable_year": s.taxable_year,
                    "quarter": s.quarter,
                    "month": s.month,
                    "status": format!("{:?}", s.status),
                    "updated_at": s.updated_at,
                })
            })
            .collect();
        serialize(&items)
    })
}

/// Move a saved draft into the queue. Generic forms go through
/// `QueueableKind` + `save_queued_queueable`; the two engine-guarded forms
/// use their dedicated transition + persistence, mirroring the painted views.
fn queue_form_draft(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    let queued = with_database(|db| {
        let queued: Result<i64, HostError> = (|| match form_code.as_str() {
            "2551Q" => crate::screen_form_module::out_of_line(|| {
                let q = quarter.ok_or_else(|| {
                    HostError::new("2551Q drafts are quarterly: pass quarter 1-4")
                })?;
                db.confirm_initial_profile_year(&tin, i32::from(year))
                    .map_err(|error| HostError::new(error.to_string()))?;
                let mut draft = db
                    .get_2551q_draft(&tin, year, q)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No saved 2551Q draft for that period"))?;
                if let Some(profile) = db
                    .get_profile_by_tin(&tin)
                    .map_err(|error| HostError::new(error.to_string()))?
                {
                    // A stored resolution error clears once a confirmed version
                    // covers the quarter. The transition still runs the page's
                    // Validate (`filing_errors`).
                    let _ = draft.reconcile_with_effective_profile(&profile);
                    // Queued with the penalties of the Calendar's deadline now.
                    draft.apply_penalty_profile(&profile, &db.get_deadline_overrides());
                }
                draft.transition_to_queued().map_err(|errors| {
                    HostError::new(
                        errors
                            .iter()
                            .map(|(field, msg)| format!("{field}: {msg}"))
                            .collect::<Vec<_>>()
                            .join("; "),
                    )
                })?;
                db.save_queued_2551q_draft_and_election(&draft)
                    .map_err(|error| HostError::new(error.to_string()))
            }),
            "1601C" => crate::screen_form_module::out_of_line(|| {
                let month = quarter
                    .ok_or_else(|| HostError::new("1601C drafts are monthly: pass the month"))?;
                let mut draft = db
                    .get_1601c_draft(&tin, year, month)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No saved 1601C draft for that period"))?;
                // Queued with the penalties of the Calendar's deadline now.
                if let Some(profile) = db
                    .get_profile_by_tin(&tin)
                    .map_err(|error| HostError::new(error.to_string()))?
                {
                    draft.apply_penalty_profile(&profile, &db.get_deadline_overrides());
                }
                draft.transition_to_queued().map_err(|errors| {
                    HostError::new(
                        errors
                            .iter()
                            .map(|(field, msg)| format!("{field}: {msg}"))
                            .collect::<Vec<_>>()
                            .join("; "),
                    )
                })?;
                db.save_queued_1601c_draft(&draft)
                    .map_err(|error| HostError::new(error.to_string()))
            }),
            _ => {
                use bir_core::forms::queueable::QueueableKind;
                let kind = QueueableKind::from_form_code(&form_code)
                    .ok_or_else(|| HostError::new(format!("Form {form_code} is not queueable")))?;
                bir_core::with_queueable_kind!(kind, F => crate::screen_form_module::out_of_line(|| {
                    let mut draft: F = db
                        .get_form_draft(&tin, &form_code, year, quarter)
                        .map_err(|error| HostError::new(error.to_string()))?
                        .ok_or_else(|| {
                            HostError::new(format!("No saved {form_code} draft for that period"))
                        })?;
                    draft
                        .queue(bir_core::filing_queue::QueueAuthSource::Gui)
                        .map_err(|errors| {
                            HostError::new(
                                errors
                                    .iter()
                                    .map(|(field, msg)| format!("{field}: {msg}"))
                                    .collect::<Vec<_>>()
                                    .join("; "),
                            )
                        })?;
                    db.save_queued_queueable(&draft)
                        .map_err(|error| HostError::new(error.to_string()))
                }))
            }
        })();
        queued.map(|id| HostValue::Number(id as f64))
    })?;
    wake_submission_worker();
    Ok(queued)
}

/// Cancel an unclaimed queued submission back to Draft (same guards as the
/// painted views: claimed or submitted work cannot be canceled this way).
fn cancel_queued(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    let cancelled = with_database(|db| {
        let result: Result<(), HostError> = (|| match form_code.as_str() {
            "2551Q" => crate::screen_form_module::out_of_line(|| {
                let q = quarter.ok_or_else(|| {
                    HostError::new("2551Q drafts are quarterly: pass quarter 1-4")
                })?;
                let draft = db
                    .get_2551q_draft(&tin, year, q)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No queued 2551Q draft for that period"))?;
                db.cancel_queued_2551q_submission(&draft)
                    .map_err(|error| HostError::new(error.to_string()))?;
                Ok(())
            }),
            "1601C" => crate::screen_form_module::out_of_line(|| {
                let month = quarter
                    .ok_or_else(|| HostError::new("1601C drafts are monthly: pass the month"))?;
                let draft = db
                    .get_1601c_draft(&tin, year, month)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No queued 1601C draft for that period"))?;
                db.cancel_queued_1601c_submission(&draft)
                    .map_err(|error| HostError::new(error.to_string()))?;
                Ok(())
            }),
            _ => {
                use bir_core::forms::queueable::QueueableKind;
                let kind = QueueableKind::from_form_code(&form_code)
                    .ok_or_else(|| HostError::new(format!("Form {form_code} is not queueable")))?;
                bir_core::with_queueable_kind!(kind, F => crate::screen_form_module::out_of_line(|| {
                    let draft: F = db
                        .get_form_draft(&tin, &form_code, year, quarter)
                        .map_err(|error| HostError::new(error.to_string()))?
                        .ok_or_else(|| {
                            HostError::new(format!("No queued {form_code} draft for that period"))
                        })?;
                    db.cancel_queued_queueable(&draft)
                        .map_err(|error| HostError::new(error.to_string()))?;
                    Ok(())
                }))
            }
        })();
        result.map(|_| HostValue::Null)
    })?;
    wake_submission_worker();
    Ok(cancelled)
}

/// The painted Background Tasks page merges `job_queue` rows with every
/// queued filing submission — same two sources, same label via
/// `FilingJobLabel`.
fn list_background_jobs() -> HostResult {
    with_database(|db| {
        let mut items: Vec<serde_json::Value> = db
            .list_jobs()
            .map_err(|error| HostError::new(error.to_string()))?
            .iter()
            .map(|job| {
                serde_json::json!({
                    "kind": "job",
                    "id": job.id,
                    "name": job.name,
                    "job_type": job.job_type,
                    "cron_expr": job.cron_expr,
                    "status": job.status,
                    "retries": job.retries,
                    "last_run_at": job.last_run_at,
                    "next_run_at": job.next_run_at,
                    "created_at": job.created_at,
                    "output_log": job.output_log,
                })
            })
            .collect();
        let emails_by_tin: std::collections::HashMap<String, String> = db
            .list_profiles()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|profile| {
                profile
                    .tracking_mailbox()
                    .map(|email| (profile.tin.full(), email.to_string()))
            })
            .collect();
        for sum in db
            .list_all_queued_submissions()
            .map_err(|error| HostError::new(error.to_string()))?
        {
            let email = emails_by_tin.get(&sum.tin).map(String::as_str);
            items.push(serde_json::json!({
                "kind": "submission",
                "id": sum.id,
                "tin": sum.tin,
                "form_code": sum.form_code,
                "taxable_year": sum.taxable_year,
                "quarter": sum.quarter,
                "month": sum.month,
                "name": bir_core::job_display::FilingJobLabel::from_summary(&sum, email)
                    .submit_name(),
                "job_type": "System",
                "cron_expr": null,
                "status": format!("{:?}", sum.status),
                "retries": 0,
                "last_run_at": null,
                "next_run_at": null,
                "created_at": sum.updated_at,
                "output_log": null,
            }));
        }
        serialize(&items)
    })
}

fn delete_background_job(arguments: &gpui_shell::HostArguments) -> HostResult {
    let id = arguments.integer(0)?;
    with_database(|db| {
        db.delete_job(id)
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Null)
    })
}

/// Archive a job the painted way — status `Archived` via `save_job`.
fn archive_background_job(arguments: &gpui_shell::HostArguments) -> HostResult {
    let id = arguments.integer(0)?;
    with_database(|db| {
        let mut job = db
            .list_jobs()
            .map_err(|error| HostError::new(error.to_string()))?
            .into_iter()
            .find(|job| job.id == Some(id))
            .ok_or_else(|| HostError::new("No job with that id"))?;
        job.status = "Archived".to_string();
        db.save_job(job)
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Null)
    })
}

fn purge_archived_jobs() -> HostResult {
    with_database(|db| {
        db.delete_archived_jobs()
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Null)
    })
}

/// A JavaScript screen failure. `console.*` already reaches tracing as
/// `gpui_shell::script`; this is the path a caught screen error takes when
/// it never calls `console` (a failed host read, a render throw).
fn record_screen_log(arguments: &gpui_shell::HostArguments) -> HostResult {
    let level = arguments.string(0)?.to_owned();
    let message = arguments.string(1)?.to_owned();
    crate::logging::record_screen_log(&level, &message);
    Ok(HostValue::Null)
}

/// Today's rotated log (`ebirforms.YYYY-MM-DD.log`), tailed like the painted
/// Logs tab — it keeps at most the newest 5,000 lines.
pub(crate) fn job_log_tail() -> HostResult {
    let logs_path = bir_core::log_files::today_log_path(&bir_core::log_files::log_dir());
    let file_name = logs_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (content, lines) = match std::fs::read_to_string(&logs_path) {
        Ok(text) => {
            let total = text.lines().count();
            let tail = if total <= 5_000 {
                text
            } else {
                text.lines()
                    .skip(total - 5_000)
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            (tail, total)
        }
        Err(_) => ("No log entries yet today.".to_string(), 0),
    };
    serialize(&serde_json::json!({
        "file_name": file_name,
        "lines": lines,
        "content": content,
    }))
}

/// The frozen print-sheet slug each form's painted `PRINT_SLUG` names.
pub(crate) fn print_slug_for(form_code: &str) -> Option<&'static str> {
    Some(match form_code {
        "0605" => "0605-1999",
        "0619E" => "0619e-2018",
        "0619F" => "0619f-2018",
        "1600PT" => "1600-pt-2018",
        "1600VT" => "1600-vt-2018",
        "1600WP" => "1600wp-2010",
        "1601C" => "1601c-2018",
        "1601EQ" => "1601eq-2019",
        "1601FQ" => "1601-fq-2020",
        "1602Q" => "1602q-2019",
        "1603Q" => "1603q-2018",
        "1604C" => "1604c-2018",
        "1604E" => "1604e-2018",
        "1604F" => "1604f-2018",
        "1606" => "1606-2018",
        "1700" => "1700-2018",
        "1701" => "1701-2018",
        "1701A" => "1701a-2018",
        "1701MS" => "1701ms-2024",
        "1701Q" => "1701q-2018",
        "1702EX" => "1702ex-2018",
        "1702MX" => "1702mx-2018c",
        "1702Q" => "1702q-2018",
        "1702RT" => "1702rt-2018c",
        "1706" => "1706-2018",
        "1707" => "1707-2021",
        "1707A" => "1707a-2021",
        "1800" => "1800-2018",
        "1801" => "1801-2018",
        "2000" => "2000-dst-2018",
        "2000OT" => "2000-ot-2018",
        "2200A" => "2200a-2020",
        "2200AN" => "2200an-2018",
        "2200C" => "2200c-2018",
        "2200M" => "2200m-2018",
        "2200P" => "2200p-2020",
        "2200S" => "2200s-2018",
        "2200T" => "2200t-2022",
        "2550M" => "2550m-2007",
        "2550Q" => "2550q-2024",
        "2551Q" => "2551q-2018",
        "2552" => "2552-2018",
        "2553" => "2553-1999",
        _ => return None,
    })
}

/// The receipt page a confirmed draft appends — the shell port of the painted
/// `form_html_preview_launcher::receipt_page_for`.
pub(crate) fn receipt_page(
    db: &Database,
    receipt_id: Option<i64>,
    mailbox: &str,
) -> Option<bir_print::frozen_html::ReceiptPage> {
    let receipt_id = receipt_id?;
    let receipt = db.get_submission_receipt_by_id(receipt_id).ok().flatten()?;
    Some(bir_print::frozen_html::ReceiptPage {
        filename: receipt.filename.clone(),
        subject: "Tax Return Receipt Confirmation".to_string(),
        from: receipt
            .source_from
            .clone()
            .unwrap_or_else(|| "ebirforms-noreply@bir.gov.ph".to_string()),
        to: mailbox.to_string(),
        received_at: bir_core::background_cron::display_received_at(
            &receipt.received_date,
            &receipt.received_time,
        ),
        email_received_at: receipt
            .email_received_at
            .as_deref()
            .and_then(bir_core::receipt::display_email_received_at),
        body_text: receipt.raw_text.clone(),
        body_html: receipt
            .raw_html
            .as_deref()
            .map(bir_core::receipt::sanitized_receipt_html),
    })
}

/// The stored draft's print field map (writer key -> value) and, for a
/// confirmed return, its receipt page: what `form_print_html` fills the
/// frozen sheet with.
pub(crate) fn print_fields(
    db: &Database,
    tin: &str,
    form_code: &str,
    year: u16,
    quarter: Option<u8>,
) -> Result<
    (
        std::collections::BTreeMap<String, String>,
        Option<bir_print::frozen_html::ReceiptPage>,
    ),
    HostError,
> {
    match form_code {
        "2551Q" => crate::screen_form_module::out_of_line(|| {
            let q = quarter.ok_or_else(|| HostError::new("2551Q drafts are quarterly"))?;
            let draft = db
                .get_2551q_draft(tin, year, q)
                .map_err(|error| HostError::new(error.to_string()))?
                .ok_or_else(|| HostError::new("No saved 2551Q draft for that period"))?;
            let page = receipt_page(db, draft.receipt_id, &draft.email);
            Ok((draft.to_print_field_map(), page))
        }),
        "1601C" => crate::screen_form_module::out_of_line(|| {
            let month = quarter.ok_or_else(|| HostError::new("1601C drafts are monthly"))?;
            let draft = db
                .get_1601c_draft(tin, year, month)
                .map_err(|error| HostError::new(error.to_string()))?
                .ok_or_else(|| HostError::new("No saved 1601C draft for that period"))?;
            let page = receipt_page(db, draft.receipt_id, &draft.email_address);
            Ok((draft.to_print_field_map(), page))
        }),
        _ => {
            use bir_core::forms::queueable::QueueableKind;
            let kind = QueueableKind::from_form_code(form_code)
                .ok_or_else(|| HostError::new(format!("Form {form_code} is not printable")))?;
            bir_core::with_queueable_kind!(kind, F => crate::screen_form_module::out_of_line(|| {
                let draft: F = db
                    .get_form_draft(tin, form_code, year, quarter)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| {
                        HostError::new(format!("No saved {form_code} draft for that period"))
                    })?;
                let page = receipt_page(
                    db,
                    draft.lifecycle().receipt_id,
                    draft.submission_email(),
                );
                Ok((draft.print_field_map(), page))
            }))
        }
    }
}

/// Fill the form's frozen print sheet, write it to a temp HTML file, and open
/// it with the platform's default handler (the painted app shows it in a wry
/// preview window — the shell has no wry, and `cx.open_url` is HTTP-only, so
/// the printable equivalent is the system browser via xdg-open/open/start).
fn form_print_html(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    let slug = print_slug_for(&form_code)
        .ok_or_else(|| HostError::new(format!("Form {form_code} has no print sheet")))?;
    with_database(|db| {
        let (fields, receipt) = print_fields(db, &tin, &form_code, year, quarter)?;
        let profile = db
            .get_profile_by_tin(&tin)
            .ok()
            .flatten()
            .map(|p| p.full_name.clone());
        open_print_preview(slug, &form_code, year, &fields, receipt.as_ref(), profile)
    })
}

/// Fill `slug`'s frozen sheet with `fields`, write it to a temp HTML file and
/// open it in the system browser. Answers `{ path }`.
pub(crate) fn open_print_preview(
    slug: &str,
    form_code: &str,
    year: u16,
    fields: &std::collections::BTreeMap<String, String>,
    receipt: Option<&bir_print::frozen_html::ReceiptPage>,
    profile_name: Option<String>,
) -> HostResult {
    let (html, title) = print_preview_page(slug, form_code, fields, receipt, profile_name)?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let path =
        std::env::temp_dir().join(format!("bir-{form_code}-{year}-{timestamp}-preview.html"));
    std::fs::write(&path, html).map_err(|error| HostError::new(error.to_string()))?;
    // In-app preview window (Blitz view + hidden WebKit PDF for Print/Share);
    // one window per form, re-opening refreshes it in place.
    crate::print_preview_window::request_keyed(path.clone(), title, form_code.to_string());
    Ok(HostObject::new()
        .field("path", path.display().to_string())
        .into())
}

/// `slug`'s frozen sheet filled with `fields` as the preview shows it, and
/// the preview's title ("BIR Form 2551Q — <taxpayer>"). Opens and writes
/// nothing.
pub(crate) fn print_preview_page(
    slug: &str,
    form_code: &str,
    fields: &std::collections::BTreeMap<String, String>,
    receipt: Option<&bir_print::frozen_html::ReceiptPage>,
    profile_name: Option<String>,
) -> Result<(String, String), HostError> {
    let html = bir_print::frozen_html::filled_document_with_receipt(slug, fields, receipt)
        .map_err(HostError::new)?;
    // A4 layout + static markup (no scripts, values as text): prints one A4
    // page per form page from any engine; `print-overrides/<slug>.css` on top.
    let html =
        crate::static_html::staticize(&crate::print_sheet::prepare_html_for(slug, &html, true));
    let title = match profile_name.filter(|n| !n.trim().is_empty()) {
        Some(name) => format!("BIR Form {form_code} \u{2014} {name}"),
        None => format!("BIR Form {form_code}"),
    };
    Ok((html, title))
}

/// Launch the platform's default HTML viewer on `path` — detached, so the
/// host call returns immediately like the painted preview window does.
#[allow(dead_code)]
fn open_in_system_browser(path: &std::path::Path) -> Result<(), HostError> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args([
            "/c",
            "start",
            "",
            path.as_os_str().to_string_lossy().as_ref(),
        ]);
        cmd
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(path);
        cmd
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(path);
        cmd
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| HostError::new(error.to_string()))
}

/// Close the live connection — used before a database-replace import so the
/// next `with_database` reopens the freshly extracted file (and re-acquires
/// the owner lock the closed connection was holding). The submission worker
/// shares the handle, so the connection is swapped for an in-memory
/// placeholder inside it rather than the handle being dropped: dropping it
/// would leave the worker holding the old file and its owner lock.
fn reset_database() -> Result<(), HostError> {
    DATABASE.with(|slot| {
        let slot = slot.borrow();
        let Some(handle) = slot.as_ref() else {
            return Ok(());
        };
        let placeholder = Database::open_ephemeral().map_err(db_error)?;
        *handle
            .lock()
            .map_err(|_| HostError::new("database lock poisoned"))? = placeholder;
        RELEASED.set(true);
        forget_global_hotkey();
        Ok(())
    })
}

/// The page's import. The host always opens the file dialog (the script has
/// none), so the page cannot choose the file: a call that passes any
/// argument is refused before anything opens. Tests and scripted imports
/// call [`import_archive_from`] instead.
fn import_archive(arguments: &gpui_shell::HostArguments) -> HostResult {
    if !arguments.is_empty() {
        return Err(HostError::new(
            "import_archive takes no path; the host opens the file dialog",
        ));
    }
    let Some(path) = rfd::FileDialog::new()
        .set_title("Select an exported .zip archive")
        .add_filter("Export Archive", &["zip"])
        .pick_file()
    else {
        return Ok(HostObject::new().field("kind", "cancelled").into());
    };
    import_archive_from(&path)
}

/// Import the archive at `path`; not a verb. Archive handling stays
/// host-side and mirrors `ImportExportView`: a zip with `bir_data.db` is a
/// full database backup, one with any `profile.json` — top-level, or under
/// `Profiles/<TIN>/` in a Settings all-profiles backup — is a profile
/// export, anything else is invalid.
fn import_archive_from(path: &std::path::Path) -> HostResult {
    // What `import_profile_data` accepts: any `profile.json` — top-level
    // (one profile) or under `Profiles/<TIN>/` (the Settings all-profiles
    // backup). A database backup's marker is a top-level `bir_data.db`.
    let entry_names: Vec<String> = std::fs::File::open(path)
        .ok()
        .and_then(|file| zip::ZipArchive::new(file).ok())
        .map(|mut archive| {
            (0..archive.len())
                .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
                .collect()
        })
        .unwrap_or_default();
    let is_db = entry_names.iter().any(|name| name == "bir_data.db");
    let is_profile = entry_names
        .iter()
        .any(|name| name.ends_with("profile.json"));

    if is_db {
        // Painted flow (import_export.rs): safety-backup the live file, then
        // extract the archive over it. Drop the cached connection first — it
        // holds the owner lock and an open handle on the file being replaced.
        let db_path = db::app_database_path();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0);
        if db_path.exists()
            && let Some(parent) = db_path.parent()
        {
            // Flush the WAL first: a raw copy of bir_data.db alone can miss
            // commits still living in -wal, and this copy is the restore
            // point if the extract corrupts the database.
            with_database(|db| db.checkpoint().map_err(db_error))?;
            let _ = std::fs::copy(
                &db_path,
                parent.join(format!("bir_data.safety-{timestamp}.db")),
            );
        }
        reset_database()?;
        return match bir_core::import::extract_database_zip(path, &db_path) {
            Ok(()) => Ok(HostObject::new().field("kind", "database").into()),
            Err(error) => Err(HostError::new(error.to_string())),
        };
    }

    if is_profile {
        return with_database(|db| match bir_core::import::import_profile_data(db, path) {
            Ok(()) => Ok(HostObject::new().field("kind", "profile").into()),
            Err(error) => Err(HostError::new(error.to_string())),
        });
    }

    Ok(HostObject::new().field("kind", "invalid").into())
}

/// The page's export: `tin`'s profile as a `.zip` archive under the host's
/// save dialog. The page never chooses the file — the same rule as
/// `import_archive` — so anything after `(tin, pin)`, or a `pin` that is
/// not a string or `null`, is refused before anything opens. Tests and
/// scripted exports call [`export_profile_to`] instead.
///
/// A taxpayer with a PIN exports only under it: the PIN is checked under
/// the same rate limiter as [`verify_taxpayer_pin`], and a refusal —
/// "This profile's PIN is required to export it." or "Wrong PIN." —
/// happens before the dialog, so no archive exists without authorization.
/// A taxpayer without a PIN exports ungated; an authenticator alone is not
/// a PIN.
fn export_profile(arguments: &gpui_shell::HostArguments) -> HostResult {
    if arguments.len() > 2 {
        return Err(HostError::new(
            "export_profile takes no path; the host opens the save dialog",
        ));
    }
    let tin = arguments.string(0)?.to_string();
    let pin = match arguments.get(1) {
        None | Some(HostValue::Null) => None,
        Some(HostValue::Str(pin)) => Some(pin.clone()),
        Some(_) => {
            return Err(HostError::new(
                "export_profile takes no path; the host opens the save dialog",
            ));
        }
    };
    // The profile names its taxpayer's root, the way `set_profile_pin`
    // resolves a branch TIN; an unknown TIN is refused before the dialog.
    let tin_root =
        with_database(|db| Ok(get_profile_by_tin(db, &tin)?.tin.full()[..9].to_string()))?;
    let authorized = auth_limiter::guarded(&auth_limiter::taxpayer(&tin_root), || {
        with_database(|db| {
            let credentials = taxpayer_credentials(db, &tin_root)?;
            match credentials.pin_hash {
                // Nothing to check — an authenticator alone is not a PIN —
                // and nothing counts against the limiter.
                None => Ok(true),
                Some(_) if pin.is_none() => Err(HostError::new(
                    "This profile's PIN is required to export it.",
                )),
                Some(hash) => Ok(pin
                    .as_deref()
                    .is_some_and(|pin| pin_matches(Some(hash.as_str()), pin))),
            }
        })
    })?;
    if !authorized {
        return Err(HostError::new("Wrong PIN."));
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let Some(path) = rfd::FileDialog::new()
        .set_title("Save the profile archive")
        .set_file_name(format!("BIR_Profile_{}_{}.zip", tin, timestamp))
        .add_filter("Export Archive", &["zip"])
        .save_file()
    else {
        return Ok(HostObject::new().field("kind", "cancelled").into());
    };
    export_profile_to(&tin, pin.as_deref(), &path)
}

/// Write `tin`'s profile archive to `path`; not a verb. The PIN gate is
/// bir-core's: [`export_profile_data`](bir_core::export_profile_data)
/// refuses a PIN'd taxpayer without its PIN before the file exists, so a
/// refusal here leaves nothing on disk either.
fn export_profile_to(tin: &str, pin: Option<&str>, path: &std::path::Path) -> HostResult {
    with_database(|db| {
        bir_core::export_profile_data(db, tin, path, pin).map_err(db_error)?;
        Ok(HostObject::new()
            .field("kind", "exported")
            .field("path", path.display().to_string())
            .into())
    })
}

/// Tax deadlines feed for the calendar page.
fn list_deadlines() -> HostResult {
    with_database(|db| {
        db.list_tax_deadlines()
            .map_err(|error| HostError::new(error.to_string()))
            .and_then(|deadlines| serialize(&deadlines))
    })
}

/// Pick up route-3 filings the account server is holding for this computer.
///
/// Each row is gated by the standing send OK and by [`crate::agent::send_draft`],
/// which runs the page's Validate before queueing. The server never contacts BIR.
#[cfg(not(feature = "agent"))]
fn pull_device_filings() -> HostResult {
    Ok(HostValue::Null)
}

#[cfg(feature = "agent")]
fn pull_device_filings() -> HostResult {
    pull_device_filings_agent()
}

#[cfg(feature = "agent")]
fn pull_device_filings_agent() -> HostResult {
    let Some((origin, token)) = crate::cloud_account::device_poll_auth() else {
        return Ok(HostValue::Null);
    };
    let client = bir_core::loopback::localhost_ipv4_first(reqwest::blocking::Client::builder())
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|error| HostError::new(error.to_string()))?;
    let url = format!("{}/api/device-filings", origin.trim_end_matches('/'));
    let response = match client
        .get(&url)
        .header("authorization", format!("Bearer {token}"))
        .send()
    {
        Ok(response) if response.status().is_success() => response,
        _ => return Ok(HostValue::Null),
    };
    let text = response.text().unwrap_or_default();
    let body: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    let Some(filings) = body.get("filings").and_then(|value| value.as_array()) else {
        return Ok(HostValue::Null);
    };
    let mut message: Option<String> = None;
    for filing in filings {
        let id = filing
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let outcome = with_database(|db| accept_device_filing(db, filing).map_err(HostError::new))?;
        let status = outcome
            .get("status")
            .and_then(|value| value.as_str())
            .unwrap_or("failed");
        // A missing grant leaves the server row queued. Invalid and picked
        // are reported so the assistant can read them.
        if status != "approval_required" {
            let report = if status == "queued" { "picked" } else { status };
            let payload = serde_json::json!({
                "status": report,
                "error_summary": outcome.get("message").and_then(|value| value.as_str()).unwrap_or(""),
                "field_errors": outcome.get("field_errors").cloned().unwrap_or(serde_json::json!([])),
            });
            let _ = client
                .post(format!("{url}/{id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(payload.to_string())
                .send();
        }
        if status == "queued" {
            wake_submission_worker();
            message = Some(
                outcome
                    .get("message")
                    .and_then(|value| value.as_str())
                    .unwrap_or("Picked up a queued filing.")
                    .to_string(),
            );
        } else if status == "approval_required" {
            message = Some(
                "A filing is waiting. Grant a standing OK on the taxpayer page before it can upload."
                    .to_string(),
            );
        } else if status == "invalid" {
            message = Some(
                outcome
                    .get("message")
                    .and_then(|value| value.as_str())
                    .unwrap_or("A queued filing failed validation.")
                    .to_string(),
            );
        }
    }
    match message {
        Some(message) => serialize(&serde_json::json!({"message": message, "icon": "send"})),
        None => Ok(HostValue::Null),
    }
}

#[cfg(feature = "agent")]
fn accept_device_filing(
    db: &bir_core::db::Database,
    filing: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    use bir_core::forms::agent::{self, AgentFormKind};
    let code = filing
        .get("form_code")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let kind = AgentFormKind::from_code(code).ok_or_else(|| format!("unknown form `{code}`"))?;
    let tin = filing
        .get("tin")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let year = filing
        .get("tax_year")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u16;
    let period = filing
        .get("period")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u8;
    if !db
        .has_send_grant(&tin, kind.form_code())
        .map_err(|error| error.to_string())?
    {
        return Ok(serde_json::json!({
            "ok": false,
            "status": "approval_required",
            "pending": true,
            "queued": false,
            "filed": false,
            "form": kind.form_code(),
            "message": "A filing is waiting for a standing OK.",
        }));
    }
    let profile = db
        .get_profile_by_tin(&tin)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("no taxpayer profile for {tin}"))?;
    let mut draft =
        crate::screen_form_module::load_draft(db, kind, &profile, year, period, false, None)?;
    if let Some(fields) = filing.get("fields").and_then(|value| value.as_object()) {
        let mut fill_errors = Vec::new();
        for (key, value) in fields {
            if let Err(message) = agent::fill(kind, &mut draft, key, value, None) {
                fill_errors.push(serde_json::json!({"field": key, "message": message}));
            }
        }
        if !fill_errors.is_empty() {
            return Ok(serde_json::json!({
                "ok": false,
                "status": "invalid",
                "queued": false,
                "filed": false,
                "form": kind.form_code(),
                "field_errors": fill_errors,
                "message": "A box could not be filled. Nothing was queued.",
            }));
        }
    }
    crate::agent::send_draft(db, kind, &mut draft)
}

/// The script is on the main window (`open`) or back on the gate, first
/// launch, or the sign-in screen. Rebuilds the menu bar to match.
fn shell_set_chrome(arguments: &gpui_shell::HostArguments) -> HostResult {
    let open = arguments.boolean(0)?;
    crate::app_menu::set_shell_chrome_open(open);
    let _ = gpui_shell::with_current_app(crate::app_menu::refresh);
    Ok(HostValue::Null)
}

/// Next command the agent host queued for the script, or null.
///
/// The TCP thread never touches the window. It posts work; the GPUI timer
/// runs the host op and pushes a command here; the script polls this.
fn agent_take() -> HostResult {
    // Queued UI commands are tools. A locked gate must not apply them.
    crate::account_gate::ensure_unlocked()?;
    #[cfg(feature = "agent")]
    let command = crate::agent::pop_command();
    #[cfg(not(feature = "agent"))]
    let command: Option<serde_json::Value> = None;
    match command {
        Some(value) => Ok(HostValue::Str(value.to_string())),
        None => Ok(HostValue::Null),
    }
}

/// The module the app's JavaScript imports as `"bir"`.
pub fn module() -> HostModule {
    let module = HostModule::new("bir")
        .function("system_info", |_| system_info())
        .function("set_global_hotkey", set_global_hotkey)
        .function("set_log_retention", set_log_retention)
        .function("update_status", |_| update_status())
        .function("install_update", |_| install_update())
        .function("dismiss_update", |_| dismiss_update())
        .function("check_for_updates", |_| check_for_updates())
        .function("set_auto_update_download", set_auto_update_download)
        .function("list_profiles", |_| list_profiles())
        .function("forms_catalog", |_| forms_catalog())
        .function("get_setting", get_setting)
        .function("set_setting", set_setting)
        .function("profile_detail", profile_detail)
        .function("save_profile", save_profile)
        .function("set_profile_archived", set_profile_archived)
        .function("delete_profile", delete_profile)
        .function("set_profile_pin", set_profile_pin)
        .function("set_profile_totp", set_profile_totp)
        .function("auth_state", |_| auth_state())
        .function("taxpayer_auth", taxpayer_auth)
        .function("set_taxpayer_pin", set_taxpayer_pin)
        .function("set_taxpayer_totp", set_taxpayer_totp)
        .function("verify_taxpayer_pin", verify_taxpayer_pin)
        .function("verify_taxpayer_totp", verify_taxpayer_totp)
        .function("verify_admin_pin", verify_admin_pin)
        .function("verify_admin_totp", verify_admin_totp)
        .function("set_admin_pin", set_admin_pin)
        .function("set_admin_totp", set_admin_totp)
        .function("setup_solo", setup_solo)
        .function("setup_helpdesk", setup_helpdesk)
        .function("change_owner", change_owner)
        .function("change_to_helpdesk", change_to_helpdesk)
        .function("create_recovery_key", |_| create_recovery_key())
        .function("use_recovery_key", use_recovery_key)
        .async_function("mac_login", mac_login)
        .function("set_taxpayer_archived", set_taxpayer_archived)
        .function("delete_taxpayer", delete_taxpayer)
        .function("generate_totp_secret", generate_totp_secret)
        .function("delete_setting", delete_setting)
        .function("list_drafts", list_drafts)
        .function("get_form_draft", get_form_draft)
        .function("save_form_draft", save_form_draft)
        .function("list_queued", |_| list_queued())
        .function("queue_form_draft", queue_form_draft)
        .function("cancel_queued", cancel_queued)
        .function("list_deadlines", |_| list_deadlines())
        .function("import_archive", import_archive)
        .function("export_profile", export_profile)
        .function("form_print_html", form_print_html)
        .function("list_background_jobs", |_| list_background_jobs())
        .function("delete_background_job", delete_background_job)
        .function("archive_background_job", archive_background_job)
        .function("purge_archived_jobs", |_| purge_archived_jobs())
        .function("job_log_tail", |_| job_log_tail())
        .function("record_screen_log", record_screen_log)
        .function("agent_take", |_| agent_take())
        .function("pull_device_filings", |_| pull_device_filings())
        .function("shell_set_chrome", shell_set_chrome);
    let module = crate::cloud_account::register(module);
    let module = crate::tax_profile_module::register(module);
    let module = crate::screen_form_module::register(module);
    let module = crate::dashboard_module::register(module);
    module.declarations(format!(
        "{BASE_DECLARATIONS}{}{}{}{}",
        crate::cloud_account::DECLARATIONS,
        crate::tax_profile_module::DECLARATIONS,
        crate::screen_form_module::DECLARATIONS,
        crate::dashboard_module::DECLARATIONS,
    ))
}

/// TypeScript for the functions registered in this file. The tax-profile and
/// screen-form files append their own (`declarations` replaces, so `module`
/// concatenates them).
const BASE_DECLARATIONS: &str = r#"
export interface HotkeyPreset {
    value: string;
    label: string;
}
export interface SystemInfo {
    os: string;
    arch: string;
    app_version: string;
    database_path: string;
    data_dir: string;
    logs_dir: string;
    /** Null when no global show chord is stored. */
    global_hotkey: string | null;
    hotkey_presets: HotkeyPreset[];
    log_retention_days: number;
    log_retention_choices: number[];
    runtime: string;
}
export interface ProfileSummary {
    id: number;
    tin: string;
    full_name: string;
    rdo_code: string;
    email: string;
    taxpayer_type: string;
    is_vat_registered: boolean;
    is_archived: boolean;
    default_form_type: string;
    applicable_forms: string[];
}
export interface FormCatalogEntry {
    code: string;
    title: string;
    category: string;
    frequency: string;
    requires_employees: boolean;
    is_deprecated: boolean;
}
export function system_info(): SystemInfo;
/** Empty string clears the chord. An unknown key is refused. */
export function set_global_hotkey(combo: string): null;
/** Days of daily log files to keep. Out of range is refused. */
export function set_log_retention(days: number): null;

export interface UpdateStatus {
    /** idle, checking, downloading, ready, failed, up_to_date, available, or installing. */
    phase: string;
    /** The release that was found, or null. */
    version: string | null;
    /** A sentence for a failure or "already current", or null. */
    message: string | null;
    /** Whether a newer release downloads without asking. */
    auto_download: boolean;
    /** The version compiled into this binary. */
    current_version: string;
}
export function update_status(): UpdateStatus;
/** Replace this install with the verified download and relaunch. */
export function install_update(): null;
/** Hide the update prompt until the next check. */
export function dismiss_update(): null;
/** Look for a release now, and download it even when auto-download is off. */
export function check_for_updates(): null;
/** `auto_update_download`. Off is `0`; anything else leaves downloading on. */
export function set_auto_update_download(enabled: boolean): null;
export function list_profiles(): ProfileSummary[];
export function forms_catalog(): FormCatalogEntry[];
export function get_setting(key: string): string | null;
export function set_setting(key: string, value: string): null;

export interface AuthState {
    /** null on a fresh install that has not chosen yet. */
    mode: "solo" | "helpdesk" | null;
    /** solo: the owner's TIN root; helpdesk: "000000000"; null if unset. */
    admin_root: string | null;
    /** solo: the owner taxpayer's PIN; helpdesk: the admin PIN setting. */
    has_admin_pin: boolean;
    has_admin_totp: boolean;
    has_recovery_key: boolean;
    /** Touch ID / account password (macOS) or Windows Hello can confirm the admin. */
    mac_login_available: boolean;
}
export interface ProfileAuth {
    has_pin: boolean;
    has_totp: boolean;
}
export interface TotpSecret {
    secret: string;
    qr_path: string;
}
export interface RecoveryKey {
    /** Shown once, e.g. "7K3M-Q9TB-0XWD-H2RE"; only its hash is kept. */
    recovery_key: string;
}
export function profile_detail(tin: string): object;
export function save_profile(profile: object): object;
export function set_profile_archived(tin: string, archived: boolean): object;
export function delete_profile(tin: string): null;
/** Sets the PIN of the branch's taxpayer (one PIN per TIN root). */
export function set_profile_pin(tin: string, pin: string | null): null;
/** Sets the authenticator of the branch's taxpayer; setting needs a current code. */
export function set_profile_totp(tin: string, secret: string | null, token?: string): null;
export function auth_state(): AuthState;
/** PIN and authenticator of a taxpayer (TIN root), shared by every branch. */
export function taxpayer_auth(tin_root: string): ProfileAuth;
/** pin: exactly 4 digits, or null to clear. */
export function set_taxpayer_pin(tin_root: string, pin: string | null): null;
/** Set (with a current code from the app) or clear (null) a taxpayer's authenticator. */
export function set_taxpayer_totp(tin_root: string, secret: string | null, code?: string): null;
export function verify_taxpayer_pin(tin_root: string, pin: string): boolean;
export function verify_taxpayer_totp(tin_root: string, code: string): boolean;
/** solo: the owner taxpayer's PIN; helpdesk: the admin PIN. false when not set up. */
export function verify_admin_pin(pin: string): boolean;
export function verify_admin_totp(code: string): boolean;
/** Help desk refuses to clear the PIN while there is no admin authenticator. */
export function set_admin_pin(pin: string | null): null;
/** Setting needs a current code; help desk refuses to clear it while there is no admin PIN. */
export function set_admin_totp(secret: string | null, code?: string): null;
/** First launch, after creating the owner's taxpayer. Refused once set up. */
export function setup_solo(tin_root: string): null;
/** First launch for a help desk: admin PIN plus a new recovery key. Refused once set up. */
export function setup_helpdesk(pin: string): RecoveryKey;
/** Admin only: make this taxpayer the owner (solo), from solo or help desk. Needs the current admin PIN or code; the taxpayer needs a PIN or authenticator and an active branch. Help desk's admin PIN and authenticator are removed. Answers a new recovery key. */
export function change_owner(tin_root: string, pin: string | null, code: string | null): RecoveryKey;
/** Admin only: switch from solo to help desk with a new admin PIN. Needs the current admin PIN or code. Answers a new recovery key. */
export function change_to_helpdesk(pin: string | null, code: string | null, new_pin: string): RecoveryKey;
/** Replace the admin recovery key. Needs the app set up. */
export function create_recovery_key(): RecoveryKey;
/** Single use: true (and the key is forgotten) on a match; case, dashes and spaces ignored. */
export function use_recovery_key(key: string): boolean;
/** Touch ID / account password (Windows Hello on Windows); false elsewhere or when cancelled. */
export function mac_login(reason: string): Promise<boolean>;
/** Archive or unarchive every branch. Archiving the admin taxpayer is refused. */
export function set_taxpayer_archived(tin_root: string, archived: boolean): null;
/** Delete the taxpayer and every branch. Refused for the admin taxpayer. */
export function delete_taxpayer(tin_root: string): null;
export function generate_totp_secret(account: string): TotpSecret;
export function delete_setting(key: string): null;

export interface DraftSummary {
    id: number;
    tin: string;
    form_code: string;
    taxable_year: number;
    quarter: number | null;
    month: number | null;
    status: "Draft" | "Queued" | "Submitted" | "Confirmed" | "Paid";
    updated_at: string;
}
export function list_drafts(tin: string, year: number): DraftSummary[];
export function get_form_draft(tin: string, form_code: string, year: number, quarter: number | null): object | null;
export function save_form_draft(tin: string, form_code: string, year: number, quarter: number | null, status: string, data: object): number;
export function list_queued(): DraftSummary[];
export function queue_form_draft(tin: string, form_code: string, year: number, quarter: number | null): number;
export function cancel_queued(tin: string, form_code: string, year: number, quarter: number | null): null;
export interface TaxDeadline {
    id: number | null;
    form_type: string;
    due_date: string;
    description: string;
}
export function list_deadlines(): TaxDeadline[];
export interface ImportResult {
    kind: "database" | "profile" | "invalid" | "cancelled";
}
export function import_archive(): ImportResult;
export interface ExportResult {
    kind: "exported" | "cancelled";
    path?: string;
}
/** The taxpayer's profile archive under the host's save dialog. A taxpayer with a PIN needs it as `pin`; the page never passes a path — the host chooses the file. */
export function export_profile(tin: string, pin?: string | null): ExportResult;
export interface PrintPreview {
    path: string;
}
export function form_print_html(tin: string, form_code: string, year: number, quarter: number | null): PrintPreview;
export interface BackgroundJob {
    kind: "job" | "submission";
    id: number | null;
    tin?: string;
    form_code?: string;
    taxable_year?: number;
    quarter?: number | null;
    month?: number | null;
    name: string;
    job_type: string;
    cron_expr: string | null;
    status: string;
    retries: number;
    last_run_at: string | null;
    next_run_at: string | null;
    created_at: string;
    output_log: string | null;
}
export function list_background_jobs(): BackgroundJob[];
export function delete_background_job(id: number): null;
export function archive_background_job(id: number): null;
export function purge_archived_jobs(): null;
export interface JobLogTail {
    file_name: string;
    lines: number;
    content: string;
}
export function job_log_tail(): JobLogTail;
/** A screen failure (`error` / `warn` / `info` / `debug`) for the Logs tab and the daily log file. Secrets are redacted. */
export function record_screen_log(level: string, message: string): null;
/** Next UI command from the gpui-agent host, or null when none is waiting. */
export function agent_take(): string | null;
/** Pick up filings an assistant queued on the account server. Null when there is nothing to say. */
export function pull_device_filings(): { message: string; icon: string } | null;
/** True once the main window is up. False through the gate, first launch, and sign-in. */
export function shell_set_chrome(open: boolean): null;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::profile::{PROFILE_CREDENTIAL_KEYS, TaxpayerType};
    use bir_core::tax_profile::{BranchFacts, NewTaxpayer};
    use gpui_shell::HostArguments;

    const TIN: &str = "12345678800000";

    #[test]
    fn locked_state_refuses_data_verbs() {
        crate::account_gate::force_locked(|| {
            let error = list_profiles().expect_err("profiles are filing data");
            assert_eq!(error.message(), crate::account_gate::LOCKED_MESSAGE);
            let error = auth_state().expect_err("auth state names taxpayers");
            assert_eq!(error.message(), crate::account_gate::LOCKED_MESSAGE);
            let error = with_database(|_| Ok(())).expect_err("database is locked");
            assert_eq!(error.message(), crate::account_gate::LOCKED_MESSAGE);
            let error = agent_take().expect_err("agent commands are tools");
            assert_eq!(error.message(), crate::account_gate::LOCKED_MESSAGE);
            assert!(system_info().is_ok());
        });
    }

    fn setup() {
        use_ephemeral_database();
        with_database(|db| {
            db.create_taxpayer(NewTaxpayer {
                tin_root: "123456788".into(),
                name: "Example Trading Corp".into(),
                taxpayer_type: TaxpayerType::Corporation,
                birth_date: None,
                head_office: BranchFacts {
                    rdo_code: "040".into(),
                    registered_address: "1 Example Street, Pasig".into(),
                    zip_code: "1600".into(),
                    phone: "0280000000".into(),
                    email: "office@example.com".into(),
                },
            })
            .map_err(|error| HostError::new(error.to_string()))?;
            Ok(())
        })
        .unwrap();
    }

    fn args(values: Vec<HostValue>) -> HostArguments {
        HostArguments::new(values)
    }

    /// Round 3, item 4: the generic save never stores a client's
    /// `trigger_date_entered`; a stored flag for the same date is kept.
    #[test]
    fn the_generic_save_keeps_only_the_stored_trigger_flag() {
        setup();
        let save = |data: serde_json::Value| {
            save_form_draft(&args(vec![
                TIN.into(),
                "1800".into(),
                2026_i64.into(),
                1_i64.into(),
                "Draft".into(),
                json(data),
            ]))
            .unwrap()
        };
        let stored = |field: &str| {
            with_database(|db| {
                Ok(db
                    .get_form_draft::<serde_json::Value>(TIN, "1800", 2026, Some(1))
                    .unwrap()
                    .unwrap()[field]
                    .clone())
            })
            .unwrap()
        };
        let gift = serde_json::json!({
            "donation_month": 9, "donation_day": 15, "donation_year": 2026,
            "created_at": "2026-10-01T00:00:00+00:00",
            "trigger_date_entered": true,
        });
        save(gift.clone());
        assert_eq!(stored("trigger_date_entered"), serde_json::Value::Null);
        // A flag stored for this date (as the page's own save leaves it) is
        // kept on a generic save of the same date, and dropped when the
        // date changes.
        with_database(|db| {
            let mut flagged = gift.clone();
            flagged["trigger_date_entered"] = serde_json::json!(true);
            db.save_form_draft(
                TIN,
                "1800",
                2026,
                Some(1),
                &bir_core::forms::FilingStatus::Draft,
                &flagged,
            )
            .unwrap();
            Ok(())
        })
        .unwrap();
        save(gift.clone());
        assert_eq!(stored("trigger_date_entered"), true);
        // Another draft (created_at) with the same date does not inherit it.
        let mut other = gift.clone();
        other["created_at"] = serde_json::json!("2026-10-02T00:00:00+00:00");
        save(other);
        assert_eq!(stored("trigger_date_entered"), serde_json::Value::Null);
        save(gift.clone());
        assert_eq!(stored("trigger_date_entered"), serde_json::Value::Null);
        let mut moved = gift;
        moved["donation_day"] = serde_json::json!(16);
        save(moved);
        assert_eq!(stored("trigger_date_entered"), serde_json::Value::Null);
        // The marker is host state too (P5-7): a forged `true` on an
        // unmarked stored row does not persist, and a payload that omits
        // the key keeps a stored row's marker. The row is `gift` again
        // (the moved date above was the last save).
        let mut gift = serde_json::json!({
            "donation_month": 9, "donation_day": 15, "donation_year": 2026,
            "created_at": "2026-10-01T00:00:00+00:00",
            "trigger_date_entered": true,
        });
        save(gift.clone());
        let mut forged = gift.clone();
        forged["trigger_date_model"] = serde_json::json!(true);
        save(forged);
        assert_eq!(stored("trigger_date_model"), serde_json::Value::Null);
        with_database(|db| {
            let mut marked = gift.clone();
            marked["trigger_date_model"] = serde_json::json!(true);
            marked["trigger_date_entered"] = serde_json::json!(true);
            db.save_form_draft(
                TIN,
                "1800",
                2026,
                Some(1),
                &bir_core::forms::FilingStatus::Draft,
                &marked,
            )
            .unwrap();
            Ok(())
        })
        .unwrap();
        // A payload that omits the lifecycle keys keeps the stored row's.
        gift.as_object_mut().unwrap().remove("trigger_date_entered");
        save(gift);
        assert_eq!(stored("trigger_date_model"), true);
        assert_eq!(stored("trigger_date_entered"), true);
    }

    /// Round 5, item 6: on an open-ended form the generic save with no
    /// entry adds a new one, so it checks that (absent) row, not entry 1:
    /// a 1606 sale copied from a flagged entry 1 does not inherit its flag.
    #[test]
    fn the_generic_save_checks_the_row_it_writes() {
        setup();
        let sale = serde_json::json!({
            "transaction_month": 9, "transaction_day": 18, "transaction_year": 2026,
            "created_at": "2026-10-01T00:00:00+00:00",
            "trigger_date_entered": true,
            "trigger_parts_typed": ["transaction_day", "transaction_month", "transaction_year"],
        });
        with_database(|db| {
            db.save_form_draft(
                TIN,
                "1606",
                2026,
                Some(1),
                &bir_core::forms::FilingStatus::Draft,
                &sale,
            )
            .unwrap();
            Ok(())
        })
        .unwrap();
        save_form_draft(&args(vec![
            TIN.into(),
            "1606".into(),
            2026_i64.into(),
            HostValue::Null,
            "Draft".into(),
            json(sale.clone()),
        ]))
        .unwrap();
        let rows: Vec<String> = with_database(|db| {
            let mut stmt = db
                .conn_for_tests()
                .prepare("SELECT data_json FROM form_drafts WHERE form_code = '1606' ORDER BY id")
                .unwrap();
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            Ok(rows)
        })
        .unwrap();
        assert_eq!(rows.len(), 2, "a new entry");
        assert!(
            rows[0].contains("trigger_date_entered"),
            "entry 1 as stored"
        );
        assert!(!rows[1].contains("trigger_date_entered"), "{}", rows[1]);
        assert!(!rows[1].contains("trigger_parts_typed"), "{}", rows[1]);
    }

    /// The 2551Q and 1601C arms store no trigger flag. This holds because
    /// `Form2551QDraft` and `Form1601CDraft` have no such field: the arms
    /// overlay the client's data on a fresh typed draft, and serde drops
    /// the unknown key. It fails if either draft type gains the field
    /// without its arm settling it.
    #[test]
    fn the_typed_arms_of_the_generic_save_store_no_trigger_flag() {
        setup();
        for (code, slot) in [("2551Q", 1_i64), ("1601C", 1)] {
            save_form_draft(&args(vec![
                TIN.into(),
                code.into(),
                2026_i64.into(),
                slot.into(),
                "Draft".into(),
                json(serde_json::json!({ "trigger_date_entered": true })),
            ]))
            .unwrap_or_else(|error| panic!("{code}: {error}"));
            let stored: String = with_database(|db| {
                Ok(db
                    .conn_for_tests()
                    .query_row(
                        "SELECT data_json FROM form_drafts WHERE form_code = ?1",
                        [code],
                        |row| row.get(0),
                    )
                    .unwrap())
            })
            .unwrap();
            assert!(!stored.contains("trigger_date_entered"), "{code}: {stored}");
        }
    }

    fn has_totp() -> bool {
        with_database(|db| Ok(taxpayer_credentials(db, ROOT)?.totp_secret.is_some())).unwrap()
    }

    fn current_code(secret: &str) -> String {
        use totp_rs::{Algorithm, Secret, TOTP};
        let bytes = Secret::Encoded(secret.to_string()).to_bytes().unwrap();
        TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes, None, String::new())
            .unwrap()
            .generate_current()
            .unwrap()
    }

    const ROOT: &str = "123456788";
    const BRANCH: &str = "12345678800001";
    const SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";

    fn s(value: &str) -> HostValue {
        HostValue::Str(value.into())
    }

    #[test]
    fn global_hotkey_round_trip_rejects_an_unknown_key() {
        setup();
        let info = to_json(&system_info().unwrap());
        assert!(info["global_hotkey"].is_null());
        assert!(
            info["logs_dir"].as_str().unwrap().ends_with("logs"),
            "{}",
            info["logs_dir"]
        );
        // Linux is ~/.taxman-ebir. macOS is the App Group container, Windows
        // is %LOCALAPPDATA%\Taxman\eBIRForms. The field must be that path.
        assert_eq!(
            info["data_dir"].as_str().unwrap(),
            bir_core::platform::data_dir().display().to_string()
        );
        assert!(info["hotkey_presets"].as_array().unwrap().len() >= 4);

        set_global_hotkey(&args(vec![s("control+alt+E")])).unwrap();
        assert_eq!(
            to_json(&system_info().unwrap())["global_hotkey"],
            "control+alt+E"
        );
        // The tray's read (cached after the first one) sees the new chord.
        assert_eq!(stored_global_hotkey().as_deref(), Some("control+alt+E"));

        let refused = set_global_hotkey(&args(vec![s("control+notakey")])).unwrap_err();
        assert!(refused.message().contains("not one"));
        assert_eq!(
            to_json(&system_info().unwrap())["global_hotkey"],
            "control+alt+E",
            "a refused chord leaves the stored one"
        );

        set_global_hotkey(&args(vec![s("  ")])).unwrap();
        assert!(to_json(&system_info().unwrap())["global_hotkey"].is_null());
        assert_eq!(stored_global_hotkey(), None);
    }

    #[test]
    fn log_retention_round_trip_refuses_out_of_range() {
        setup();
        set_log_retention(&args(vec![30_i64.into()])).unwrap();
        assert_eq!(to_json(&system_info().unwrap())["log_retention_days"], 30);

        let refused = set_log_retention(&args(vec![0_i64.into()])).unwrap_err();
        assert!(refused.message().contains("between"));
        assert_eq!(
            to_json(&system_info().unwrap())["log_retention_days"],
            30,
            "a refused value leaves the stored one"
        );

        let refused = set_log_retention(&args(vec![400_i64.into()])).unwrap_err();
        assert!(refused.message().contains("between"));
        assert_eq!(to_json(&system_info().unwrap())["log_retention_days"], 30);
    }

    fn add_branch() {
        with_database(|db| {
            db.create_branch(
                ROOT,
                bir_core::tax_profile::NewBranch {
                    branch_code: "00001".into(),
                    name: "Makati branch".into(),
                    fields: Default::default(),
                },
            )
            .map_err(|error| HostError::new(error.to_string()))?;
            Ok(())
        })
        .unwrap();
    }

    fn profile_credentials(tin: &str) -> (Option<String>, Option<String>) {
        with_database(|db| {
            let profile = get_profile_by_tin(db, tin)?;
            Ok((
                profile.profile_pin_hash.clone(),
                profile.totp_secret.clone(),
            ))
        })
        .unwrap()
    }

    fn truthy(result: HostResult) -> bool {
        result.unwrap() == HostValue::Bool(true)
    }

    fn state() -> serde_json::Value {
        to_json(&auth_state().unwrap())
    }

    fn set(key: &str, value: &str) {
        with_database(|db| db.set_setting(key, value).map_err(db_error)).unwrap();
    }

    #[test]
    fn set_taxpayer_totp_needs_a_matching_code_and_clears_with_null() {
        setup();
        let wrong = set_taxpayer_totp(&args(vec![s(ROOT), s(SECRET), s("000000")]));
        if current_code(SECRET) != "000000" {
            assert!(wrong.unwrap_err().message().contains("does not match"));
            assert!(!has_totp());
        }
        set_taxpayer_totp(&args(vec![s(ROOT), s(SECRET), s(&current_code(SECRET))])).unwrap();
        assert!(has_totp());
        assert!(truthy(verify_taxpayer_totp(&args(vec![
            s(ROOT),
            s(&current_code(SECRET))
        ]))));
        set_taxpayer_totp(&args(vec![s(ROOT), HostValue::Null])).unwrap();
        assert!(!has_totp());
    }

    #[test]
    fn credentials_are_stored_once_per_taxpayer_not_on_branches() {
        setup();
        add_branch();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        assert_eq!(profile_credentials(TIN), (None, None));
        assert_eq!(profile_credentials(BRANCH), (None, None));
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        assert!(!truthy(verify_taxpayer_pin(&args(vec![
            s(ROOT),
            s("1234")
        ]))));
        assert_eq!(
            to_json(&taxpayer_auth(&args(vec![s(ROOT)])).unwrap()),
            serde_json::json!({"has_pin": true, "has_totp": false})
        );

        // The legacy per-branch setter writes the same taxpayer PIN.
        set_profile_pin(&args(vec![s(BRANCH), s("2222")])).unwrap();
        assert_eq!(profile_credentials(BRANCH), (None, None));
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("2222")]))));

        assert!(set_taxpayer_pin(&args(vec![s(ROOT), s("12345")])).is_err());
        set_taxpayer_pin(&args(vec![s(ROOT), HostValue::Null])).unwrap();
        assert!(!truthy(verify_taxpayer_pin(&args(vec![
            s(ROOT),
            s("2222")
        ]))));
    }

    #[test]
    fn six_wrong_pins_make_even_the_right_one_wait() {
        setup();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        for _ in 0..5 {
            assert!(!truthy(verify_taxpayer_pin(&args(vec![
                s(ROOT),
                s("1111")
            ]))));
        }
        let sixth = verify_taxpayer_pin(&args(vec![s(ROOT), s("1111")])).unwrap_err();
        assert!(
            sixth.to_string().contains("Too many wrong tries"),
            "{sixth}"
        );
        assert!(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).is_err());
    }

    #[test]
    fn auth_state_is_unset_until_first_launch_setup() {
        setup();
        assert_eq!(
            state(),
            serde_json::json!({
                "mode": null,
                "admin_root": null,
                "has_admin_pin": false,
                "has_admin_totp": false,
                "has_recovery_key": false,
                "mac_login_available": mac_login_available(),
            })
        );
        assert!(!truthy(verify_admin_pin(&args(vec![s("0817")]))));
        assert!(set_admin_pin(&args(vec![s("0817")])).is_err());
        assert!(create_recovery_key().is_err());
    }

    const OTHER_ROOT: &str = "987654321";

    fn add_other_taxpayer() {
        with_database(|db| {
            db.create_taxpayer(NewTaxpayer {
                tin_root: OTHER_ROOT.into(),
                name: "Maria Santos".into(),
                taxpayer_type: TaxpayerType::Individual,
                birth_date: None,
                head_office: BranchFacts {
                    rdo_code: "043".into(),
                    registered_address: "2 Sample Road, Makati".into(),
                    zip_code: "1200".into(),
                    phone: String::new(),
                    email: String::new(),
                },
            })
            .map_err(|error| HostError::new(error.to_string()))?;
            Ok(())
        })
        .unwrap();
    }

    fn null() -> HostValue {
        HostValue::Null
    }

    fn message(result: HostResult) -> String {
        result.unwrap_err().message().to_owned()
    }

    fn key_of(result: HostResult) -> String {
        to_json(&result.unwrap())["recovery_key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn change_owner_moves_the_admin_to_another_taxpayer() {
        setup();
        add_other_taxpayer();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        let old_key = key_of(create_recovery_key());

        let to_other = |pin: &str| change_owner(&args(vec![s(OTHER_ROOT), s(pin), null()]));
        assert!(message(to_other("1111")).contains("not the admin"));
        assert!(message(to_other("0817")).contains("no PIN or authenticator"));
        set_taxpayer_pin(&args(vec![s(OTHER_ROOT), s("2468")])).unwrap();
        with_database(|db| db.set_taxpayer_archived(OTHER_ROOT, true).map_err(db_error)).unwrap();
        assert!(message(to_other("0817")).contains("archived"));
        with_database(|db| {
            db.set_taxpayer_archived(OTHER_ROOT, false)
                .map_err(db_error)
        })
        .unwrap();
        assert!(change_owner(&args(vec![s("000000000"), s("0817"), null()])).is_err());
        assert!(message(change_owner(&args(vec![s(ROOT), s("0817"), null()]))).contains("already"));
        assert_eq!(state()["admin_root"], ROOT, "refusals change nothing");

        let new_key = key_of(to_other("0817"));
        assert_eq!(state()["mode"], "solo");
        assert_eq!(state()["admin_root"], OTHER_ROOT);
        assert!(truthy(verify_admin_pin(&args(vec![s("2468")]))));
        assert!(!truthy(verify_admin_pin(&args(vec![s("0817")]))));
        // The old owner keeps their own PIN and is now an ordinary taxpayer.
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])).unwrap();
        assert!(set_taxpayer_archived(&args(vec![s(OTHER_ROOT), HostValue::Bool(true)])).is_err());
        // The previous admin's recovery key is dead; the new one works once.
        assert!(!truthy(use_recovery_key(&args(vec![s(&old_key)]))));
        assert!(truthy(use_recovery_key(&args(vec![s(&new_key)]))));
    }

    #[test]
    fn wrong_admin_codes_at_a_setup_change_count_toward_the_lockout() {
        setup();
        add_other_taxpayer();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        set_taxpayer_pin(&args(vec![s(OTHER_ROOT), s("2468")])).unwrap();
        let attempt = |pin: &str| change_owner(&args(vec![s(OTHER_ROOT), s(pin), null()]));
        for _ in 0..5 {
            assert!(message(attempt("1111")).contains("not the admin"));
        }
        assert!(message(attempt("1111")).contains("Too many wrong tries"));
        // Locked: even the right PIN waits, and nothing changed.
        assert!(message(attempt("0817")).contains("Too many wrong tries"));
        assert!(
            verify_admin_pin(&args(vec![s("0817")])).is_err(),
            "admin sign-in shares the same count"
        );
        assert_eq!(state()["admin_root"], ROOT);
    }

    #[test]
    fn change_to_helpdesk_and_back_to_an_owner() {
        setup();
        add_other_taxpayer();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        set_admin_totp(&args(vec![s(SECRET), s(&current_code(SECRET))])).unwrap();

        assert!(
            message(change_to_helpdesk(&args(vec![
                s("1111"),
                null(),
                s("4321")
            ])))
            .contains("not the admin")
        );
        assert!(change_to_helpdesk(&args(vec![s("0817"), null(), s("12")])).is_err());
        assert_eq!(state()["mode"], "solo", "refusals change nothing");

        // Confirming with the authenticator code works as well as the PIN.
        let key = key_of(change_to_helpdesk(&args(vec![
            null(),
            s(&current_code(SECRET)),
            s("4321"),
        ])));
        assert_eq!(key.len(), 19);
        assert_eq!(state()["mode"], "helpdesk");
        assert_eq!(state()["admin_root"], "000000000");
        assert_eq!(
            state()["has_admin_totp"],
            false,
            "no authenticator carried over"
        );
        assert!(truthy(verify_admin_pin(&args(vec![s("4321")]))));
        assert!(!truthy(verify_admin_pin(&args(vec![s("0817")]))));
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        // The former owner is no longer protected from archiving.
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])).unwrap();
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(false)])).unwrap();
        assert!(
            message(change_to_helpdesk(&args(vec![
                s("4321"),
                null(),
                s("5555")
            ])))
            .contains("already")
        );

        // Back to solo with another taxpayer as the owner.
        set_taxpayer_pin(&args(vec![s(OTHER_ROOT), s("2468")])).unwrap();
        assert!(
            change_owner(&args(vec![s(OTHER_ROOT), s("0817"), null()])).is_err(),
            "help desk PIN needed"
        );
        change_owner(&args(vec![s(OTHER_ROOT), s("4321"), null()])).unwrap();
        assert_eq!(state()["mode"], "solo");
        assert_eq!(state()["admin_root"], OTHER_ROOT);
        assert!(truthy(verify_admin_pin(&args(vec![s("2468")]))));
        let leftover = with_database(|db| Ok(setting(db, ADMIN_PIN_HASH)?.is_some())).unwrap();
        assert!(!leftover, "the help desk admin PIN is removed");
    }

    #[test]
    fn solo_admin_is_the_owner_taxpayer() {
        setup();
        assert!(setup_solo(&args(vec![s("000000000")])).is_err());
        assert!(
            setup_solo(&args(vec![s("999999999")])).is_err(),
            "no such taxpayer"
        );
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        assert!(setup_solo(&args(vec![s(ROOT)])).is_err(), "only once");
        assert!(setup_helpdesk(&args(vec![s("1111")])).is_err(), "only once");
        assert_eq!(state()["mode"], "solo");
        assert_eq!(state()["admin_root"], ROOT);

        // One code: the owner's taxpayer PIN is the admin PIN.
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        assert_eq!(state()["has_admin_pin"], true);
        assert!(truthy(verify_admin_pin(&args(vec![s("0817")]))));
        set_admin_pin(&args(vec![s("4321")])).unwrap();
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("4321")]))));
        set_admin_totp(&args(vec![s(SECRET), s(&current_code(SECRET))])).unwrap();
        assert!(has_totp());
        assert!(truthy(verify_admin_totp(&args(vec![s(&current_code(
            SECRET
        ))]))));
        // The help desk settings play no part in solo mode.
        set(ADMIN_PIN_HASH, &bir_core::crypto::hash_pin("9999"));
        assert!(!truthy(verify_admin_pin(&args(vec![s("9999")]))));

        // Clearing the owner's PIN and authenticator is allowed in solo.
        set_admin_pin(&args(vec![HostValue::Null])).unwrap();
        set_admin_totp(&args(vec![HostValue::Null])).unwrap();
        assert_eq!(state()["has_admin_pin"], false);
        assert_eq!(state()["has_admin_totp"], false);
    }

    #[test]
    fn helpdesk_admin_uses_the_settings_and_keeps_one_way_in() {
        setup();
        assert!(setup_helpdesk(&args(vec![s("12")])).is_err());
        let created = to_json(&setup_helpdesk(&args(vec![s("0817")])).unwrap());
        assert_eq!(created["recovery_key"].as_str().unwrap().len(), 19);
        let st = state();
        assert_eq!(st["mode"], "helpdesk");
        assert_eq!(st["admin_root"], "000000000");
        assert_eq!(st["has_admin_pin"], true);
        assert_eq!(st["has_admin_totp"], false);
        assert_eq!(st["has_recovery_key"], true);
        assert!(truthy(verify_admin_pin(&args(vec![s("0817")]))));
        assert!(!truthy(verify_admin_pin(&args(vec![s("1234")]))));

        // A taxpayer's PIN is not the admin PIN in help desk mode.
        set_taxpayer_pin(&args(vec![s(ROOT), s("5555")])).unwrap();
        assert!(!truthy(verify_admin_pin(&args(vec![s("5555")]))));

        // The only way in cannot be removed.
        let refused = set_admin_pin(&args(vec![HostValue::Null])).unwrap_err();
        assert!(refused.message().contains("at least one way"));
        assert_eq!(state()["has_admin_pin"], true);

        set_admin_totp(&args(vec![s(SECRET), s(&current_code(SECRET))])).unwrap();
        assert!(truthy(verify_admin_totp(&args(vec![s(&current_code(
            SECRET
        ))]))));
        set_admin_pin(&args(vec![HostValue::Null])).unwrap();
        assert_eq!(state()["has_admin_pin"], false);
        let refused = set_admin_totp(&args(vec![HostValue::Null])).unwrap_err();
        assert!(refused.message().contains("at least one way"));
        assert_eq!(state()["has_admin_totp"], true);
    }

    #[test]
    fn a_recovery_key_works_once_and_a_new_one_replaces_it() {
        setup();
        let first = to_json(&setup_helpdesk(&args(vec![s("0817")])).unwrap())["recovery_key"]
            .as_str()
            .unwrap()
            .to_string();
        let second = to_json(&create_recovery_key().unwrap())["recovery_key"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            !truthy(use_recovery_key(&args(vec![s(&first)]))),
            "replaced"
        );
        let typed = second.to_lowercase().replace('-', " ");
        assert!(truthy(use_recovery_key(&args(vec![s(&typed)]))));
        assert_eq!(state()["has_recovery_key"], false);
        assert!(
            !truthy(use_recovery_key(&args(vec![s(&second)]))),
            "single use"
        );
    }

    #[test]
    fn the_admin_taxpayer_cannot_be_archived_or_deleted() {
        setup();
        add_branch();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        for result in [
            set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])),
            delete_taxpayer(&args(vec![s(ROOT)])),
            delete_taxpayer(&args(vec![s("000000000")])),
        ] {
            assert!(result.unwrap_err().message().contains("admin account"));
        }
        // One branch may go, but not the last one.
        delete_profile(&args(vec![s(BRANCH)])).unwrap();
        assert!(delete_profile(&args(vec![s(TIN)])).is_err());
        assert!(set_profile_archived(&args(vec![s(TIN), HostValue::Bool(true)])).is_err());
        with_database(|db| {
            assert!(db.get_taxpayer(ROOT).map_err(db_error)?.is_some());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn other_taxpayers_archive_and_delete_across_every_branch() {
        setup();
        add_branch();
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])).unwrap();
        assert!(profile_detail_archived(TIN) && profile_detail_archived(BRANCH));
        with_database(|db| {
            assert!(
                db.get_taxpayer(ROOT)
                    .map_err(db_error)?
                    .unwrap()
                    .taxpayer
                    .archived
            );
            Ok(())
        })
        .unwrap();
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(false)])).unwrap();
        assert!(!profile_detail_archived(TIN));
        delete_taxpayer(&args(vec![s(ROOT)])).unwrap();
        with_database(|db| {
            assert!(db.get_taxpayer(ROOT).map_err(db_error)?.is_none());
            assert!(db.get_profile_by_tin(BRANCH).map_err(db_error)?.is_none());
            Ok(())
        })
        .unwrap();
    }

    fn profile_detail_archived(tin: &str) -> bool {
        with_database(|db| Ok(get_profile_by_tin(db, tin)?.is_archived)).unwrap()
    }

    // ---------- the profile's credentials stay host-side ----------

    /// Distinctive values the host stores before each credential test, in
    /// `PROFILE_CREDENTIAL_KEYS` order, so a leak or an overwrite cannot
    /// pass unnoticed.
    const STORED_CREDENTIALS: [&str; 5] = [
        "stored-totp-K7Q2",
        "stored-pass-W9X4",
        "stored-access-M3R8",
        "stored-refresh-T5V1",
        "stored-pin-hash-H6N3",
    ];

    /// What a page might forge, in the same order.
    const FORGED_CREDENTIALS: [&str; 5] = [
        "forged-totp",
        "forged-pass",
        "forged-access",
        "forged-refresh",
        "forged-pin-hash",
    ];

    fn store_credentials(tin: &str) {
        with_database(|db| {
            let mut profile = get_profile_by_tin(db, tin)?;
            let [totp, password, access, refresh, pin_hash] =
                STORED_CREDENTIALS.map(|value| Some(value.to_string()));
            profile.totp_secret = totp;
            profile.imap_app_password = password;
            profile.oauth_access_token = access;
            profile.oauth_refresh_token = refresh;
            profile.profile_pin_hash = pin_hash;
            db.save_profile(profile).map_err(db_error)?;
            Ok(())
        })
        .unwrap();
    }

    /// `tin`'s five stored credentials, in `PROFILE_CREDENTIAL_KEYS` order.
    fn stored_credentials(tin: &str) -> [Option<String>; 5] {
        with_database(|db| {
            let mut profile = get_profile_by_tin(db, tin)?;
            Ok([
                profile.totp_secret.take(),
                profile.imap_app_password.take(),
                profile.oauth_access_token.take(),
                profile.oauth_refresh_token.take(),
                profile.profile_pin_hash.take(),
            ])
        })
        .unwrap()
    }

    /// Every credential stored on `tin` is still the one `store_credentials`
    /// wrote. A failure names the key, never a value.
    fn assert_credentials_kept(tin: &str, after: &str) {
        let stored = stored_credentials(tin);
        for ((key, value), expected) in PROFILE_CREDENTIAL_KEYS
            .into_iter()
            .zip(stored)
            .zip(STORED_CREDENTIALS)
        {
            assert!(
                value.as_deref() == Some(expected),
                "{after} changed the stored {key}"
            );
        }
    }

    /// `verb`'s profile answer carries neither a credential key nor any of
    /// `values` (in `PROFILE_CREDENTIAL_KEYS` order). A failure names the
    /// key, never a value.
    fn assert_no_credentials(answer: &HostValue, verb: &str, values: [&str; 5]) {
        let record = to_json(answer);
        let fields = record
            .as_object()
            .unwrap_or_else(|| panic!("{verb} answers a profile record"));
        let raw = record.to_string();
        for (key, value) in PROFILE_CREDENTIAL_KEYS.into_iter().zip(values) {
            assert!(!fields.contains_key(key), "{verb} sent the {key} key");
            assert!(!raw.contains(value), "{verb} sent the {key} value");
        }
    }

    /// The page reads a profile without its credentials: no key, no value,
    /// and the rest of the record is there.
    #[test]
    fn profile_detail_sends_no_credential() {
        setup();
        store_credentials(TIN);
        let detail = profile_detail(&args(vec![s(TIN)])).unwrap();
        assert_no_credentials(&detail, "profile_detail", STORED_CREDENTIALS);
        assert_eq!(to_json(&detail)["full_name"], "Example Trading Corp");
    }

    /// The page's round trip of `profile_detail` (a payload without the
    /// credential keys) saves its own edit and keeps all five stored values.
    #[test]
    fn save_profile_without_the_credential_keys_keeps_the_stored_ones() {
        setup();
        store_credentials(TIN);
        let mut page = to_json(&profile_detail(&args(vec![s(TIN)])).unwrap());
        for key in PROFILE_CREDENTIAL_KEYS {
            assert!(page.get(key).is_none(), "the payload carries no {key}");
        }
        page["phone"] = serde_json::json!("0281112222");
        let saved = save_profile(&args(vec![json(page)])).unwrap();
        assert_no_credentials(&saved, "save_profile", STORED_CREDENTIALS);
        assert_credentials_kept(TIN, "a save without the credential keys");
        let phone = with_database(|db| Ok(get_profile_by_tin(db, TIN)?.phone.clone())).unwrap();
        assert_eq!(phone, "0281112222", "the rest of the payload is saved");
    }

    /// Forged values leave the stored ones as they were, and so do forged
    /// nulls: the page can neither set nor clear a credential.
    #[test]
    fn save_profile_ignores_forged_credentials() {
        setup();
        store_credentials(TIN);
        let page = to_json(&profile_detail(&args(vec![s(TIN)])).unwrap());
        let (mut forged, mut nulled) = (page.clone(), page);
        for (key, value) in PROFILE_CREDENTIAL_KEYS.into_iter().zip(FORGED_CREDENTIALS) {
            forged[key] = serde_json::json!(value);
            nulled[key] = serde_json::Value::Null;
        }
        let saved = save_profile(&args(vec![json(forged)])).unwrap();
        assert_no_credentials(&saved, "save_profile", FORGED_CREDENTIALS);
        assert_no_credentials(&saved, "save_profile", STORED_CREDENTIALS);
        assert_credentials_kept(TIN, "a save with forged credentials");
        save_profile(&args(vec![json(nulled)])).unwrap();
        assert_credentials_kept(TIN, "a save with null credentials");
    }

    /// A save for a TIN with no stored row stores no credential, whatever
    /// the payload carries.
    #[test]
    fn save_profile_for_a_new_tin_stores_no_credential() {
        setup();
        let mut page = serde_json::json!({
            "id": null,
            "full_name": "Sample Holdings Inc",
            "tin": {"segment1": "987", "segment2": "654", "segment3": "320", "branch": "00000"},
            "rdo_code": "043",
            "line_of_business": "Holding company",
            "registered_address": "2 Sample Road, Makati",
            "zip_code": "1200",
            "phone": "0280000001",
            "email": "books@example.com",
            "default_form_type": "1702Q",
            "taxpayer_type": "Corporation",
        });
        for (key, value) in PROFILE_CREDENTIAL_KEYS.into_iter().zip(FORGED_CREDENTIALS) {
            page[key] = serde_json::json!(value);
        }
        let saved = save_profile(&args(vec![json(page)])).unwrap();
        assert_no_credentials(&saved, "save_profile", FORGED_CREDENTIALS);
        let stored = stored_credentials("98765432000000");
        for (key, value) in PROFILE_CREDENTIAL_KEYS.into_iter().zip(stored) {
            assert!(value.is_none(), "a new profile stored a {key}");
        }
    }

    /// A round trip that moves the profile to a new TIN writes the same row
    /// (re-keyed by its `id`), so that row's credentials are the ones kept;
    /// the new TIN has no row of its own to take them from.
    #[test]
    fn save_profile_that_moves_the_tin_keeps_the_rows_credentials() {
        setup();
        add_branch();
        store_credentials(BRANCH);
        let mut page = to_json(&profile_detail(&args(vec![s(BRANCH)])).unwrap());
        page["tin"]["branch"] = serde_json::json!("00002");
        let saved = save_profile(&args(vec![json(page)])).unwrap();
        assert_no_credentials(&saved, "save_profile", STORED_CREDENTIALS);
        assert_credentials_kept("12345678800002", "a save that moves the TIN");
        let moved =
            with_database(|db| Ok(db.get_profile_by_tin(BRANCH).map_err(db_error)?.is_none()))
                .unwrap();
        assert!(moved, "the row moved off its old TIN");
    }

    /// The third verb that answers a profile: archiving keeps the
    /// credentials stored and leaves them out of the answer.
    #[test]
    fn set_profile_archived_answers_without_credentials() {
        setup();
        store_credentials(TIN);
        let archived = set_profile_archived(&args(vec![s(TIN), HostValue::Bool(true)])).unwrap();
        assert_no_credentials(&archived, "set_profile_archived", STORED_CREDENTIALS);
        assert_eq!(to_json(&archived)["is_archived"], true);
        assert_credentials_kept(TIN, "archiving");
    }

    /// The authenticator's own writers still work: `set_taxpayer_totp` keeps
    /// the secret on the taxpayer, a page round trip of every branch profile
    /// neither shows it nor removes it, and the legacy per-branch
    /// `set_profile_totp` still clears and sets it.
    #[test]
    fn the_authenticator_writers_survive_a_profile_round_trip() {
        setup();
        add_branch();
        set_taxpayer_totp(&args(vec![s(ROOT), s(SECRET), s(&current_code(SECRET))])).unwrap();
        for tin in [TIN, BRANCH] {
            let page = profile_detail(&args(vec![s(tin)])).unwrap();
            assert!(
                !to_json(&page).to_string().contains(SECRET),
                "profile_detail sent the authenticator secret"
            );
            save_profile(&args(vec![page])).unwrap();
        }
        assert!(has_totp(), "the round trip kept the authenticator");
        assert!(truthy(verify_taxpayer_totp(&args(vec![
            s(ROOT),
            s(&current_code(SECRET))
        ]))));
        set_profile_totp(&args(vec![s(BRANCH), HostValue::Null])).unwrap();
        assert!(!has_totp());
        set_profile_totp(&args(vec![s(BRANCH), s(SECRET), s(&current_code(SECRET))])).unwrap();
        assert!(has_totp());
    }

    /// A page cannot plant a PIN. For a taxpayer without one, a PIN hash
    /// sent through `save_profile` is neither answered nor stored: the
    /// profile row's `profile_pin_hash` stays JSON null. The next launch
    /// moves only a text value onto the taxpayer (bir-core's
    /// `test_v22_moves_credentials_onto_the_taxpayer_and_clears_branch_copies`),
    /// so the taxpayer stays without a PIN.
    #[test]
    fn a_pin_hash_from_the_page_is_not_stored() {
        setup();
        let planted = bir_core::crypto::hash_pin("1234");
        let mut page = to_json(&profile_detail(&args(vec![s(TIN)])).unwrap());
        page["profile_pin_hash"] = serde_json::json!(planted);
        let saved = to_json(&save_profile(&args(vec![json(page)])).unwrap());
        assert!(
            saved.get("profile_pin_hash").is_none(),
            "save_profile sent the profile_pin_hash key"
        );
        assert!(
            !saved.to_string().contains(&planted),
            "save_profile sent the PIN hash"
        );
        let stored: Option<String> = with_database(|db| {
            Ok(db
                .conn_for_tests()
                .query_row(
                    "SELECT json_type(data_json, '$.profile_pin_hash') FROM profiles
                     WHERE tin = ?1",
                    [TIN],
                    |row| row.get(0),
                )
                .unwrap())
        })
        .unwrap();
        assert_eq!(stored.as_deref(), Some("null"), "the row's PIN hash type");
        assert_eq!(
            to_json(&taxpayer_auth(&args(vec![s(ROOT)])).unwrap()),
            serde_json::json!({"has_pin": false, "has_totp": false})
        );
        assert!(!truthy(verify_taxpayer_pin(&args(vec![
            s(ROOT),
            s("1234")
        ]))));
    }

    /// A PIN hash a profile row already holds (as an older build or the
    /// legacy profile editor leaves it until the next launch moves it onto
    /// the taxpayer) is in neither verb's answer, and a page round trip
    /// leaves it on the row.
    #[test]
    fn a_stored_pin_hash_survives_a_page_round_trip_unseen() {
        setup();
        let stored = bir_core::crypto::hash_pin("0817");
        with_database(|db| {
            let mut profile = get_profile_by_tin(db, TIN)?;
            profile.profile_pin_hash = Some(stored.clone());
            db.save_profile(profile).map_err(db_error)?;
            Ok(())
        })
        .unwrap();
        let detail = profile_detail(&args(vec![s(TIN)])).unwrap();
        let saved = save_profile(&args(vec![detail.clone()])).unwrap();
        for (verb, answer) in [("profile_detail", detail), ("save_profile", saved)] {
            let record = to_json(&answer);
            assert!(
                record.get("profile_pin_hash").is_none(),
                "{verb} sent the profile_pin_hash key"
            );
            assert!(
                !record.to_string().contains(&stored),
                "{verb} sent the PIN hash"
            );
        }
        let kept = with_database(|db| {
            let mut profile = get_profile_by_tin(db, TIN)?;
            Ok(profile.profile_pin_hash.take())
        })
        .unwrap();
        assert!(
            kept.as_deref() == Some(stored.as_str()),
            "the round trip changed the stored PIN hash"
        );
    }

    /// The PIN's own writers still work around a page round trip. Each one
    /// stores the PIN on the taxpayer (the help desk admin's in settings); a
    /// round trip of every branch profile shows no hash of it, and the PIN
    /// still verifies afterwards while the one it replaced does not. The
    /// writers: `set_taxpayer_pin`, the legacy per-branch `set_profile_pin`,
    /// `set_admin_pin` for the solo owner and for the help desk, and
    /// `change_to_helpdesk`'s new admin PIN.
    #[test]
    fn the_pin_writers_survive_a_profile_round_trip() {
        setup();
        add_branch();
        let round_trip = |pin: &str| {
            let hash = bir_core::crypto::hash_pin(pin);
            for tin in [TIN, BRANCH] {
                let page = profile_detail(&args(vec![s(tin)])).unwrap();
                assert!(
                    !to_json(&page).to_string().contains(&hash),
                    "profile_detail sent the PIN hash"
                );
                save_profile(&args(vec![page])).unwrap();
            }
        };
        let taxpayer_pin = |pin: &str| truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s(pin)])));
        let admin_pin = |pin: &str| truthy(verify_admin_pin(&args(vec![s(pin)])));

        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        round_trip("0817");
        assert!(taxpayer_pin("0817"), "set_taxpayer_pin's PIN");

        set_profile_pin(&args(vec![s(BRANCH), s("2468")])).unwrap();
        round_trip("2468");
        assert!(taxpayer_pin("2468"), "set_profile_pin's PIN");
        assert!(!taxpayer_pin("0817"), "the PIN set_profile_pin replaced");

        setup_solo(&args(vec![s(ROOT)])).unwrap();
        set_admin_pin(&args(vec![s("4321")])).unwrap();
        round_trip("4321");
        assert!(admin_pin("4321"), "set_admin_pin's solo PIN");
        assert!(taxpayer_pin("4321"), "the solo admin PIN is the owner's");
        assert!(!taxpayer_pin("2468"), "the PIN set_admin_pin replaced");

        change_to_helpdesk(&args(vec![s("4321"), null(), s("5555")])).unwrap();
        round_trip("5555");
        assert!(admin_pin("5555"), "change_to_helpdesk's admin PIN");
        assert!(
            !admin_pin("4321"),
            "the admin PIN change_to_helpdesk replaced"
        );
        assert!(taxpayer_pin("4321"), "the former owner keeps their PIN");

        set_admin_pin(&args(vec![s("6666")])).unwrap();
        round_trip("6666");
        assert!(admin_pin("6666"), "set_admin_pin's help desk PIN");
        assert!(!admin_pin("5555"), "the admin PIN set_admin_pin replaced");
    }

    // ---------- the page reaches only its own settings ----------

    /// The host's own read of `key`.
    fn stored_setting(key: &str) -> Option<String> {
        with_database(|db| setting(db, key)).unwrap()
    }

    /// `verb`'s answer for `key` is the page refusal, whose exact wording
    /// names the key and nothing stored under it. A failure prints neither
    /// the answer nor the refusal, since either could carry a stored value.
    fn assert_refused(answer: HostResult, verb: &str, key: &str) {
        match answer {
            Ok(_) => panic!("{verb} answered the page for `{key}`"),
            Err(refusal) => assert!(
                refusal.message() == format!("setting `{key}` is not available to the page"),
                "{verb} refused `{key}` in other words"
            ),
        }
    }

    /// (a) The page's one setting round-trips through the three verbs, and
    /// each step reaches the database.
    #[test]
    fn the_theme_preference_round_trips_through_the_page_verbs() {
        setup();
        let key = || s("theme_preference");
        assert_eq!(get_setting(&args(vec![key()])).unwrap(), HostValue::Null);
        assert_eq!(
            set_setting(&args(vec![key(), s("Dark")])).unwrap(),
            HostValue::Null
        );
        assert_eq!(stored_setting("theme_preference").as_deref(), Some("Dark"));
        assert_eq!(get_setting(&args(vec![key()])).unwrap(), s("Dark"));
        assert_eq!(delete_setting(&args(vec![key()])).unwrap(), HostValue::Null);
        assert_eq!(stored_setting("theme_preference"), None);
        assert_eq!(get_setting(&args(vec![key()])).unwrap(), HostValue::Null);
    }

    /// (b) The help desk admin's authenticator secret stays host-side: the
    /// page's read is refused, and the refusal carries no value.
    #[test]
    fn the_page_cannot_read_the_admin_authenticator_secret() {
        setup();
        set(ADMIN_TOTP_SECRET, SECRET);
        let answer = get_setting(&args(vec![s(ADMIN_TOTP_SECRET)]));
        assert!(
            !format!("{answer:?}").contains(SECRET),
            "get_setting sent the admin authenticator secret"
        );
        assert_refused(answer, "get_setting", ADMIN_TOTP_SECRET);
        assert!(
            stored_setting(ADMIN_TOTP_SECRET).as_deref() == Some(SECRET),
            "the refused read changed the stored secret"
        );
    }

    /// (c) The page cannot rewrite the access mode.
    #[test]
    fn the_page_cannot_rewrite_the_access_mode() {
        setup();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        let answer = set_setting(&args(vec![s(ACCESS_MODE), s("helpdesk")]));
        assert_refused(answer, "set_setting", ACCESS_MODE);
        assert_eq!(stored_setting(ACCESS_MODE).as_deref(), Some("solo"));
        assert_eq!(state()["mode"], "solo");
        assert_eq!(state()["admin_root"], ROOT);
    }

    /// (d) The page cannot delete the admin's recovery key: the stored hash
    /// is the same afterwards, and the key it belongs to still works.
    #[test]
    fn the_page_cannot_delete_the_recovery_key() {
        setup();
        let key = key_of(setup_helpdesk(&args(vec![s("0817")])));
        let before = stored_setting(RECOVERY_KEY_HASH);
        assert!(before.is_some(), "setup stores a recovery key hash");
        let answer = delete_setting(&args(vec![s(RECOVERY_KEY_HASH)]));
        assert_refused(answer, "delete_setting", RECOVERY_KEY_HASH);
        assert!(
            stored_setting(RECOVERY_KEY_HASH) == before,
            "the refused delete changed the stored recovery key hash"
        );
        assert!(truthy(use_recovery_key(&args(vec![s(&key)]))));
    }

    /// Each of the three verbs refuses every host setting, whatever is
    /// stored under it, and leaves the stored value as it was; so too a key
    /// that only looks like the page's.
    #[test]
    fn every_other_setting_is_refused_by_all_three_verbs() {
        setup();
        const STORED: &str = "stored-setting-P8D2";
        for key in [
            ACCESS_MODE,
            ADMIN_TIN_ROOT,
            ADMIN_PIN_HASH,
            ADMIN_TOTP_SECRET,
            RECOVERY_KEY_HASH,
            "Theme_Preference",
            " theme_preference",
        ] {
            set(key, STORED);
            let read = get_setting(&args(vec![s(key)]));
            assert!(
                !format!("{read:?}").contains(STORED),
                "get_setting sent the value of `{key}`"
            );
            assert_refused(read, "get_setting", key);
            assert_refused(
                set_setting(&args(vec![s(key), s("page-value")])),
                "set_setting",
                key,
            );
            assert_refused(delete_setting(&args(vec![s(key)])), "delete_setting", key);
            assert!(
                stored_setting(key).as_deref() == Some(STORED),
                "a refused verb changed `{key}`"
            );
        }
    }

    #[test]
    fn mac_login_answers_without_blocking_the_caller_off_macos() {
        if mac_login_available() {
            return; // would show a real OS prompt
        }
        let future = module()
            .begin("mac_login", &args(vec![s("unlock")]))
            .expect("sync half");
        let answer = futures::executor::block_on(future).unwrap();
        assert_eq!(answer, HostValue::Bool(false));
    }

    // ---------- import: the host always opens the file dialog ----------

    /// A directory under the system temp dir, removed when this drops.
    struct ScratchDir(std::path::PathBuf);

    impl ScratchDir {
        fn new(label: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "bir-shell-import-{label}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const ARCHIVE_TIN: &str = "98765432000000";

    fn new_taxpayer(tin_root: &str, name: &str) -> NewTaxpayer {
        NewTaxpayer {
            tin_root: tin_root.into(),
            name: name.into(),
            taxpayer_type: TaxpayerType::Corporation,
            birth_date: None,
            head_office: BranchFacts {
                rdo_code: "043".into(),
                registered_address: "2 Sample Road, Makati".into(),
                zip_code: "1200".into(),
                phone: "0280000001".into(),
                email: "books@example.com".into(),
            },
        }
    }

    /// A profile archive of `ARCHIVE_TIN` with one submission, exported
    /// from a database of its own.
    fn profile_archive(scratch: &ScratchDir) -> std::path::PathBuf {
        let source = Database::open_ephemeral().unwrap();
        source
            .create_taxpayer(new_taxpayer("987654320", "Archived Holdings Inc"))
            .unwrap();
        source
            .save_submission(bir_core::db::Submission {
                id: None,
                tin: ARCHIVE_TIN.into(),
                form_type: "1702Q".into(),
                period: "2026Q1".into(),
                status: "Draft".into(),
                form_data: Default::default(),
                submitted_at: None,
                filename: None,
                created_at: None,
                updated_at: None,
            })
            .unwrap();
        let archive = scratch.path().join("profile.zip");
        bir_core::export::export_profile_data(&source, ARCHIVE_TIN, &archive, None).unwrap();
        archive
    }

    fn archive_imported() -> bool {
        with_database(|db| {
            Ok(db
                .get_profile_by_tin(ARCHIVE_TIN)
                .map_err(db_error)?
                .is_some())
        })
        .unwrap()
    }

    /// The page cannot choose the file. The registered verb refuses a call
    /// with any argument, a path included, before a dialog opens, and
    /// imports nothing; `import_archive_from` is no verb at all.
    #[test]
    fn import_archive_refuses_any_argument() {
        setup();
        let scratch = ScratchDir::new("refused");
        let archive = profile_archive(&scratch);
        let module = module();
        assert!(module.has("import_archive"));
        assert!(!module.has("import_archive_from"), "not a verb");
        // The path first: a verb that took it would import it here instead
        // of falling through to a dialog.
        for argument in [
            s(&archive.display().to_string()),
            HostValue::Null,
            HostValue::Bool(true),
        ] {
            match module.call("import_archive", &args(vec![argument])) {
                Ok(_) => panic!("import_archive answered a call with an argument"),
                Err(refusal) => assert_eq!(
                    refusal.message(),
                    "import_archive takes no path; the host opens the file dialog"
                ),
            }
            assert!(!archive_imported(), "a refused call imported the archive");
        }
    }

    /// `import_archive_from` imports a profile archive into the open
    /// database, and answers `invalid` for a zip that is neither export.
    #[test]
    fn import_archive_from_imports_a_profile_archive() {
        setup();
        let scratch = ScratchDir::new("profile");
        let archive = profile_archive(&scratch);
        assert_eq!(
            to_json(&import_archive_from(&archive).unwrap()),
            serde_json::json!({ "kind": "profile" })
        );
        with_database(|db| {
            let profile = db
                .get_profile_by_tin(ARCHIVE_TIN)
                .map_err(db_error)?
                .expect("the imported profile");
            assert_eq!(profile.full_name, "Archived Holdings Inc");
            let submissions = db.list_submissions_for_tin(ARCHIVE_TIN).map_err(db_error)?;
            assert_eq!(submissions.len(), 1);
            assert_eq!(submissions[0].form_type, "1702Q");
            Ok(())
        })
        .unwrap();

        let other = scratch.path().join("other.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&other).unwrap());
        zip.start_file("notes.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut zip, b"not an export").unwrap();
        zip.finish().unwrap();
        assert_eq!(
            to_json(&import_archive_from(&other).unwrap()),
            serde_json::json!({ "kind": "invalid" })
        );
    }

    /// The Settings all-profiles backup puts each profile under
    /// `Profiles/<TIN>/`: `import_archive_from` reads every profile in it,
    /// not just a top-level `profile.json`.
    #[test]
    fn import_archive_from_imports_a_combined_archive() {
        setup();
        let scratch = ScratchDir::new("combined");
        let source = Database::open_ephemeral().unwrap();
        source
            .create_taxpayer(new_taxpayer("111222333", "First Combined Corp"))
            .unwrap();
        source
            .create_taxpayer(new_taxpayer("444555666", "Second Combined Corp"))
            .unwrap();
        let archive = scratch.path().join("all-profiles.zip");
        bir_core::export::export_all_profiles_data(&source, &archive).unwrap();
        drop(source);
        assert_eq!(
            to_json(&import_archive_from(&archive).unwrap()),
            serde_json::json!({ "kind": "profile" })
        );
        with_database(|db| {
            for (tin, name) in [
                ("11122233300000", "First Combined Corp"),
                ("44455566600000", "Second Combined Corp"),
            ] {
                let profile = db
                    .get_profile_by_tin(tin)
                    .map_err(db_error)?
                    .unwrap_or_else(|| panic!("the imported profile for {tin}"));
                assert_eq!(profile.full_name, name);
            }
            Ok(())
        })
        .unwrap();
    }

    /// `import_archive_from` replaces the open database with a database
    /// archive, after a safety copy of the file it replaces. That path
    /// writes the app's database file and reads its key, so it runs in a
    /// child test process whose `BIR_DATABASE_PATH` is a scratch file and
    /// whose `EBIR_TEST_ENV` keeps the key off the keychain: never the live
    /// database.
    #[test]
    fn import_archive_from_restores_a_database_archive() {
        const SCRATCH: &str = "BIR_SHELL_IMPORT_ARCHIVE_SCRATCH";
        if let Some(scratch) = std::env::var_os(SCRATCH) {
            restore_a_database_archive_in(std::path::Path::new(&scratch));
            return;
        }
        let scratch = ScratchDir::new("database");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "bir_module::tests::import_archive_from_restores_a_database_archive",
                "--nocapture",
            ])
            .current_dir(scratch.path())
            .env(SCRATCH, scratch.path())
            .env("BIR_DATABASE_PATH", scratch.path().join("bir_data.db"))
            .env("EBIR_TEST_ENV", "1")
            .env("BIR_SFTP_DRY_RUN", "1")
            .status()
            .unwrap();
        assert!(status.success(), "the database import child failed");
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("child-done"))
                .ok()
                .as_deref(),
            Some("ok"),
            "the child ran the import"
        );
    }

    /// The child's half of the test above, in the scratch environment its
    /// parent set; it refuses to run anywhere else.
    fn restore_a_database_archive_in(scratch: &std::path::Path) {
        let file = scratch.join("bir_data.db");
        assert_eq!(db::app_database_path(), file, "the scratch database");
        assert!(
            std::env::var_os("EBIR_TEST_ENV").is_some(),
            "the key stays off the keychain"
        );
        with_database(|db| {
            db.create_taxpayer(new_taxpayer("123456788", "Replaced Trading Corp"))
                .map_err(db_error)?;
            Ok(())
        })
        .unwrap();
        let source = Database::open(scratch.join("source.db")).unwrap();
        source
            .create_taxpayer(new_taxpayer("987654320", "Restored Holdings Inc"))
            .unwrap();
        source.checkpoint().unwrap();
        let archive = scratch.join("backup.zip");
        bir_core::export::export_database_zip(&source, &archive).unwrap();
        drop(source);

        assert_eq!(
            to_json(&import_archive_from(&archive).unwrap()),
            serde_json::json!({ "kind": "database" })
        );
        let safety_copies: Vec<_> = std::fs::read_dir(scratch)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("bir_data.safety-")
            })
            .collect();
        assert_eq!(safety_copies.len(), 1, "a safety copy of the replaced file");
        // The safety copy holds the just-created taxpayer because the import
        // checkpoints the WAL before copying — a raw copy alone can leave
        // committed data still in -wal, out of the file it saved.
        let safety_copy = Database::open(&safety_copies[0]).unwrap();
        assert!(
            safety_copy
                .list_profiles()
                .unwrap()
                .iter()
                .any(|profile| profile.full_name == "Replaced Trading Corp"),
            "the safety copy missed the committed profile"
        );
        drop(safety_copy);
        let tins = with_database(|db| {
            Ok(db
                .list_profiles()
                .map_err(db_error)?
                .iter()
                .map(|profile| profile.tin.full())
                .collect::<Vec<_>>())
        })
        .unwrap();
        assert_eq!(tins, [ARCHIVE_TIN], "the archive's database is open");
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
    }

    // ---------- the submission worker ----------

    /// The shell's launch starts the submission worker on the database it
    /// opens, so a queued return is sent without painted `bir` or
    /// `bir-headless` running. Opening the database reads its key, so this
    /// runs in a child test process on a scratch `BIR_DATABASE_PATH` with
    /// `EBIR_TEST_ENV` (key off the keychain) and `BIR_SFTP_DRY_RUN`: never
    /// the live database, never BIR.
    #[test]
    fn the_shell_launch_starts_the_submission_worker() {
        const SCRATCH: &str = "BIR_SHELL_SUBMISSION_WORKER_SCRATCH";
        if let Some(scratch) = std::env::var_os(SCRATCH) {
            start_the_submission_worker_in(std::path::Path::new(&scratch));
            return;
        }
        let scratch = ScratchDir::new("worker");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "bir_module::tests::the_shell_launch_starts_the_submission_worker",
                "--nocapture",
            ])
            .current_dir(scratch.path())
            .env(SCRATCH, scratch.path())
            .env("BIR_DATABASE_PATH", scratch.path().join("bir_data.db"))
            .env("EBIR_TEST_ENV", "1")
            .env("BIR_SFTP_DRY_RUN", "1")
            .status()
            .unwrap();
        assert!(status.success(), "the submission worker child failed");
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("child-done"))
                .ok()
                .as_deref(),
            Some("ok"),
            "the child ran the launch"
        );
    }

    /// The child's half of the test above. A profile with the test
    /// notification on makes every worker tick record a heartbeat alert, so
    /// the alert appearing proves the worker runs on the shell's database.
    fn start_the_submission_worker_in(scratch: &std::path::Path) {
        assert_eq!(
            db::app_database_path(),
            scratch.join("bir_data.db"),
            "the scratch database"
        );
        assert!(
            std::env::var_os("EBIR_TEST_ENV").is_some(),
            "the key stays off the keychain"
        );
        assert!(!submission_worker_started(), "nothing runs before launch");

        open_database_at_launch();
        assert!(submission_worker_started(), "the launch started the worker");

        with_database(|db| {
            db.create_taxpayer(new_taxpayer("123456788", "Worker Trading Corp"))
                .map_err(db_error)?;
            let mut profile = db
                .list_profiles()
                .map_err(db_error)?
                .into_iter()
                .next()
                .expect("the created profile");
            profile.test_notification_enabled = true;
            db.save_profile(profile).map_err(db_error)?;
            Ok(())
        })
        .unwrap();
        wake_submission_worker();

        let heartbeat = || {
            with_database(|db| {
                Ok(db
                    .list_active_alerts(None)
                    .map_err(db_error)?
                    .iter()
                    .any(|alert| alert.kind == db::alert_kinds::CRON_HEARTBEAT))
            })
            .unwrap()
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !heartbeat() {
            assert!(
                std::time::Instant::now() < deadline,
                "the submission worker never ticked on the shell's database"
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
    }

    // ---------- export_profile ----------

    /// `export_profile_to` writes `TIN`'s archive under the bir-core gate:
    /// a PIN'd taxpayer refuses — no PIN, wrong PIN — before the file
    /// exists, and the right PIN writes it.
    #[test]
    fn export_profile_to_needs_a_pinned_taxpayers_pin() {
        setup();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        let scratch = ScratchDir::new("export");
        let archive = scratch.path().join("profile.zip");

        let refusal = export_profile_to(TIN, None, &archive).unwrap_err();
        assert_eq!(
            refusal.message(),
            "This profile's PIN is required to export it."
        );
        assert!(!archive.exists(), "a refused export left a file");

        let refusal = export_profile_to(TIN, Some("1234"), &archive).unwrap_err();
        assert_eq!(refusal.message(), "Wrong PIN.");
        assert!(!archive.exists(), "a refused export left a file");

        let answer = to_json(&export_profile_to(TIN, Some("0817"), &archive).unwrap());
        assert_eq!(
            answer,
            serde_json::json!({ "kind": "exported", "path": archive.display().to_string() })
        );
        assert!(archive.exists());
    }

    /// A taxpayer without a PIN exports ungated — `None` or a stray PIN —
    /// and an authenticator alone is not a PIN.
    #[test]
    fn export_profile_to_exports_ungated_without_a_pin() {
        setup();
        let scratch = ScratchDir::new("export-open");
        let archive = scratch.path().join("profile.zip");
        assert_eq!(
            to_json(&export_profile_to(TIN, None, &archive).unwrap())["kind"],
            "exported"
        );
        assert!(archive.exists());

        set_taxpayer_totp(&args(vec![s(ROOT), s(SECRET), s(&current_code(SECRET))])).unwrap();
        let archive = scratch.path().join("totp-only.zip");
        assert_eq!(
            to_json(&export_profile_to(TIN, None, &archive).unwrap())["kind"],
            "exported"
        );
    }

    /// The page cannot choose the file. The registered verb refuses a call
    /// past `(tin, pin)` — a path included — and a non-string `pin` before
    /// a dialog opens; `export_profile_to` is no verb at all.
    #[test]
    fn export_profile_refuses_anything_past_the_pin() {
        setup();
        let module = module();
        assert!(module.has("export_profile"));
        assert!(!module.has("export_profile_to"), "not a verb");
        for call in [
            args(vec![s(TIN), s("0817"), s("/tmp/chosen.zip")]),
            args(vec![
                s(TIN),
                json(serde_json::json!({"path": "/tmp/chosen.zip"})),
            ]),
        ] {
            match module.call("export_profile", &call) {
                Ok(_) => panic!("export_profile answered a call with a path"),
                Err(refusal) => assert_eq!(
                    refusal.message(),
                    "export_profile takes no path; the host opens the save dialog"
                ),
            }
        }
    }

    /// A PIN'd taxpayer's verb refusals happen before any dialog and share
    /// `verify_taxpayer_pin`'s limiter: no PIN is the required refusal, a
    /// wrong PIN counts, six in a row lock the account — the right PIN
    /// waits — and a missing PIN is not a try.
    #[test]
    fn export_profile_wrong_pins_share_the_taxpayer_lockout() {
        setup();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        let attempt = |pin: &str| export_profile(&args(vec![s(TIN), s(pin)]));
        let none = || export_profile(&args(vec![s(TIN)]));

        assert_eq!(
            none().unwrap_err().message(),
            "This profile's PIN is required to export it."
        );
        for _ in 0..5 {
            assert_eq!(attempt("1111").unwrap_err().message(), "Wrong PIN.");
        }
        // Not a try: the required refusal still answers, not the lockout.
        assert_eq!(
            none().unwrap_err().message(),
            "This profile's PIN is required to export it."
        );
        let sixth = attempt("1111").unwrap_err();
        assert!(sixth.message().contains("Too many wrong tries"), "{sixth}");
        assert!(
            attempt("0817")
                .unwrap_err()
                .message()
                .contains("Too many wrong tries"),
            "the right PIN waits too"
        );
    }
}
