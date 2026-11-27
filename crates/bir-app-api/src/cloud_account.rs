//! Host verbs for the Buwiz account in Settings.
//!
//! The page sees account status only. Refresh tokens stay in the OS keychain
//! when Secret Service (or the macOS keychain) accepts them, and otherwise in
//! a mode-0600 file beside the database — the same fallback the database key
//! uses when the keychain is missing. The page cannot read either store.

use bir_core::cloud_account::{
    self, AccountSecrets, AccountStatus, BlockingHttp, CloudError, ENV_SERVER_URL, GateDecision,
    RefreshOutcome, SessionUpdate, StoredAccount,
};
use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};

use crate::bir_module::with_account_database;

/// Old Settings field. Startup deletes it. Nothing reads it.
const OBSOLETE_SERVER_URL_SETTING: &str = "buwiz_server_url";

pub const DECLARATIONS: &str = r#"
export interface CloudAccountStatus {
    server_url: string;
    server_url_source: string;
    /** The account site's address (where sign-in and account pages open). Empty when it is not a valid address. */
    account_url: string;
    signed_in: boolean;
    email: string | null;
    user_id: string | null;
    offline: boolean;
    practice: boolean;
    detail: string;
}
export interface CloudAccountGate {
    unlocked: boolean;
    offline: boolean;
    server_url: string;
    email: string | null;
    detail: string;
}
export function cloud_account_status(): CloudAccountStatus;
export function cloud_account_gate(): CloudAccountGate;
export function cloud_account_refresh(): Promise<CloudAccountStatus>;
export function cloud_account_sign_out(): Promise<CloudAccountStatus>;
export function cloud_account_session_opened(): null;
export function cloud_account_session_closed(): null;
/** Whether the signed-in account signs in on this device for the first time (the welcome line shows); a first time is recorded. False, and nothing recorded, when no account is signed in. The page learns nothing else: what the device keeps is a keyed hash, host-only. */
export function welcome_first_time(): boolean;
"#;

pub fn register(module: HostModule) -> HostModule {
    module
        .function("cloud_account_status", |_| status_now())
        .function("cloud_account_gate", |_| gate_value())
        .async_function("cloud_account_refresh", refresh_account)
        .async_function("cloud_account_sign_out", sign_out)
        .function("cloud_account_session_opened", |_| session_opened())
        .function("cloud_account_session_closed", |_| session_closed())
        .function("welcome_first_time", |_| welcome_first_time())
}

/// Pause filing, then open the UI only when a saved session still qualifies.
/// Call this before the menu bar is built and before the database worker starts.
pub fn prepare_launch() {
    bir_core::account_session::on_session_ended(crate::session::server_ended);
    bir_core::background_cron::set_submission_paused(true);
    forget_saved_server_url();
    drop_old_welcomed_list();
    if crate::account_gate::auth_bypass_enabled() {
        bir_core::transport::set_practice_account(false);
        crate::account_gate::set_unlocked(true);
        return;
    }
    let (unlocked, practice) = restore_saved_session();
    // Set before the gate opens, so no send can start ahead of it.
    bir_core::transport::set_practice_account(unlocked && practice);
    crate::account_gate::set_unlocked(unlocked);
    if unlocked && let Ok(Some(stored)) = load_stored() {
        // The launch resume (§A.4): a claim cut mid-move finishes here, before anything opens the
        // room. R1's order is the room open's own guard.
        crate::room_session::open_session_room(&stored);
    }
}

pub(crate) fn status_now() -> HostResult {
    let resolved = match resolve() {
        Ok(resolved) => resolved,
        Err(error) => {
            return Ok(status_value(&AccountStatus {
                server_url: rejected_origin_text(ENV_SERVER_URL, option_env!("BUWIZ_SERVER_URL")),
                server_url_source: "invalid".into(),
                signed_in: false,
                email: None,
                user_id: None,
                offline: false,
                practice: false,
                detail: error.to_string(),
            }));
        }
    };
    let stored = load_stored().ok().flatten();
    let secrets = stored
        .as_ref()
        .filter(|stored| stored.server_url == resolved.url)
        .map(StoredAccount::to_secrets);
    Ok(status_value(&cloud_account::status_from(
        &resolved,
        secrets.as_ref(),
    )))
}

/// The first tokens of a sign-in, from either path (the browser's device flow, the email code): the profile is
/// read, the sign-in saved under the account-session lock, and the device key of an earlier sign-in forgotten.
pub(crate) fn save_signed_in(
    http: &mut dyn cloud_account::Http,
    server_url: &str,
    tokens: cloud_account::TokenSet,
    now: i64,
) -> Result<(), HostError> {
    if tokens.refresh_token.is_empty() {
        return Err(HostError::new(
            "The server did not return a refresh token. Sign in again.",
        ));
    }
    let (user_id, email, practice) =
        cloud_account::load_profile(http, server_url, &tokens.access_token).map_err(host_error)?;
    let secrets = AccountSecrets {
        server_url: server_url.to_owned(),
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        access_expires_at_unix: now.saturating_add(tokens.expires_in),
        user_id,
        email,
        offline: false,
        practice,
    };
    bir_core::transport::set_practice_account(practice);
    // F10/F8': the persist and the cache are the machine's one step. Commit 3 adds the begin_open
    // inside it; register_device_soon stays here until commit moves it into commit.
    crate::session::signed_in(&stored_from(secrets, now)).map_err(host_error)?;
    bir_core::relay::forget_device();
    register_device_soon();
    Ok(())
}

/// Adds this device to the account (bir-core `relay::register_after_sign_in`) on a thread of its own, so signing in never
/// waits for it: a failure is logged there and the first send registers as it always did; a practice account adds none.
fn register_device_soon() {
    let database = bir_core::db::capture_database_path_override();
    let started = std::thread::Builder::new()
        .name("buwiz-device-register".into())
        .spawn(move || {
            database.run(bir_core::relay::register_after_sign_in);
        });
    if let Err(error) = started {
        tracing::warn!("device registration not started ({error}); the first send registers");
    }
}

/// Test seam: the Http refresh_account uses. A test parks the network by installing a factory
/// whose answer blocks on a harness gate; production never sets it, and the blocking default is
/// the real client.
#[cfg(test)]
static TEST_REFRESH_HTTP: std::sync::Mutex<
    Option<Box<dyn Fn() -> Box<dyn cloud_account::Http + Send> + Send>>,
> = std::sync::Mutex::new(None);

#[cfg(test)]
pub(crate) fn set_test_refresh_http(
    factory: Option<Box<dyn Fn() -> Box<dyn cloud_account::Http + Send> + Send>>,
) {
    *TEST_REFRESH_HTTP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = factory;
}

fn refresh_http() -> Box<dyn cloud_account::Http + Send> {
    #[cfg(test)]
    if let Some(factory) = TEST_REFRESH_HTTP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
    {
        return factory();
    }
    Box::new(BlockingHttp::new())
}

pub(crate) fn refresh_account(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    spawn_account("buwiz-account-refresh", || {
        // D2: the epoch is captured at the verb's start; the lock is held for the file reads and
        // writes only — never across refresh_session's network.
        let epoch = bir_core::session_epoch::current();
        let (stored, owner) = {
            let _one = bir_core::account_session::lock();
            let Some(stored) = load_stored()? else {
                return status_now();
            };
            let resolved = resolve()?;
            if stored.server_url != resolved.url {
                // The stored sign-in belongs to another server deployment: it ends here, through
                // the machine — the disk outside SESSION, the cache and the close inside it.
                crate::session::sign_out();
                return status_now();
            }
            let Some(owner) = bir_core::account_session::OwnerKey::of(&stored) else {
                return status_now();
            };
            (stored, owner)
        };
        let mut http = refresh_http();
        let now = cloud_account::unix_now();
        match cloud_account::refresh_session(http.as_mut(), &stored.to_secrets(), now)
            .map_err(host_error)?
        {
            SessionUpdate::Online(secrets) => {
                bir_core::transport::set_practice_account(secrets.practice);
                let _saved = {
                    let _one = bir_core::account_session::lock();
                    bir_core::account_session::save_stored_if(&owner, &stored_from(secrets, now))
                        .map_err(host_error)?
                };
            }
            SessionUpdate::Offline(secrets) => {
                bir_core::transport::set_practice_account(secrets.practice);
                let _saved = {
                    let _one = bir_core::account_session::lock();
                    bir_core::account_session::save_stored_if(
                        &owner,
                        &stored_from(secrets, stored.last_refresh_unix),
                    )
                    .map_err(host_error)?
                };
            }
            SessionUpdate::SignedOut => {
                // D1 + revision 5's A: the machine's one section — the disk's
                // delete-if-credential, the cache, the close — decided by the owner, the epoch
                // captured above, and the exact refresh token the server rejected.
                crate::session::server_ended(owner, epoch, stored.refresh_token.clone());
            }
        }
        status_now()
    })
}

fn sign_out(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    spawn_account("buwiz-account-sign-out", || {
        sign_out_with(Box::new(BlockingHttp::new()))
    })
}

/// Signs out on this device first and tells the server afterwards. The saved sign-in is gone before any
/// request starts, so an app closed while a slow or absent network holds the logout is still signed out on
/// relaunch, and the page shows the gate without waiting for the server. A sign-in this device could not
/// remove is an error the page shows, never "signed out"; the gate locks and the logout goes out either way.
fn sign_out_with(http: Box<dyn cloud_account::Http + Send>) -> HostResult {
    let _ = http;
    // D1: the end is the machine's one section — the disk first, the cache and the close inside
    // SESSION, the logout after. Nothing here writes anything itself anymore.
    crate::session::sign_out();
    crate::hooks::hooks().refresh_menu();
    status_now()
}

/// Sends the logout on a thread of its own, without the account-session lock, so a new sign-in never waits on
/// it. It carries the old sign-in's access token, which cannot end a newer sign-in, and nothing else of it. The
/// answer changes nothing here (the sign-in is already gone on this device); it is logged, without the token.
pub(crate) fn logout_soon(
    mut http: Box<dyn cloud_account::Http + Send>,
    server_url: String,
    access_token: String,
) {
    let started = std::thread::Builder::new()
        .name("buwiz-account-logout".into())
        .spawn(move || {
            match cloud_account::send_logout(http.as_mut(), &server_url, &access_token) {
                Ok(status) => tracing::info!("Buwiz logout sent: HTTP {status}"),
                Err(error) => tracing::info!("Buwiz logout not sent: {error}"),
            }
        });
    if let Err(error) = started {
        tracing::warn!("logout not sent ({error}); the sign-in is already gone on this device");
    }
}

fn session_opened() -> HostResult {
    // The page opens the gate after a sign-in; take the flag from the saved
    // account. An unreadable store leaves the value save_signed_in set.
    if let Ok(Some(stored)) = load_stored() {
        bir_core::transport::set_practice_account(stored.practice);
    }
    // A3a (§A.4), in this order: the saved sign-in is read; the legacy claim runs or resumes; the
    // account's room opens; the worker starts or wakes (the launch path below); the agent session
    // rebuilds (its generation check does, on the next batch). F2: the gate opens only after the
    // room — never ahead of it, so no tick and no screen runs on the old room under the new sign-in.
    // F10: the claim can copy, hash and zip a large legacy file — never on the UI thread, or
    // "Moving" never paints (the page polls the migration state while this runs). The order is
    // unchanged: the claim runs or resumes, the room opens, the gate opens only after the room.
    let _room_open = spawn_account("buwiz-room-open", || {
        if let Ok(Some(stored)) = load_stored() {
            crate::room_session::open_session_room(&stored);
        }
        crate::account_gate::set_unlocked(true);
        crate::bir_module::open_database_at_launch();
        crate::hooks::hooks().refresh_menu();
        Ok(HostValue::Null)
    });
    crate::hooks::hooks().start_updater();
    Ok(HostValue::Null)
}

fn session_closed() -> HostResult {
    crate::account_gate::set_unlocked(false);
    bir_core::transport::set_practice_account(false);
    crate::hooks::hooks().refresh_menu();
    Ok(HostValue::Null)
}

/// Whether the gate opens, and whether the account is a practice account.
/// Offline, the saved value stands.
fn restore_saved_session() -> (bool, bool) {
    restore_saved_session_with(&mut BlockingHttp::new())
}

pub(crate) fn restore_saved_session_with(http: &mut dyn cloud_account::Http) -> (bool, bool) {
    // D2, the launch arm: the lock is held for the reads and writes, never across the network.
    let epoch = bir_core::session_epoch::current();
    let (stored, owner) = {
        let _one = bir_core::account_session::lock();
        let Ok(Some(stored)) = load_stored() else {
            return (false, false);
        };
        let Ok(resolved) = resolve() else {
            return (false, false);
        };
        if stored.server_url != resolved.url {
            return (false, false);
        }
        let Some(owner) = bir_core::account_session::OwnerKey::of(&stored) else {
            return (false, false);
        };
        (stored, owner)
    };
    let now = cloud_account::unix_now();
    let previous_refresh = stored.last_refresh_unix;
    let mut practice = stored.practice;
    let outcome = match cloud_account::refresh_session(http, &stored.to_secrets(), now) {
        Ok(SessionUpdate::Online(secrets)) => {
            practice = secrets.practice;
            let saved = {
                let _one = bir_core::account_session::lock();
                bir_core::account_session::save_stored_if(&owner, &stored_from(secrets, now))
                    .unwrap_or(false)
            };
            if !saved {
                return (false, false);
            }
            RefreshOutcome::Online
        }
        Ok(SessionUpdate::Offline(secrets)) => {
            practice = secrets.practice;
            let _ = {
                let _one = bir_core::account_session::lock();
                bir_core::account_session::save_stored_if(
                    &owner,
                    &stored_from(secrets, previous_refresh),
                )
            };
            RefreshOutcome::Offline
        }
        Ok(SessionUpdate::SignedOut) => RefreshOutcome::Revoked,
        Err(CloudError::InvalidGrant) => RefreshOutcome::Revoked,
        Err(_) => RefreshOutcome::Offline,
    };
    match cloud_account::gate_after_refresh(outcome) {
        GateDecision::Open { .. } => {
            // The cache is the machine's picture of who is stored (F8'): the launch sets it from
            // the sign-in just confirmed.
            bir_core::account_session::cache_owner(Some(owner));
            (true, practice)
        }
        GateDecision::Closed {
            clear_secrets: true,
        } => {
            // D1 at the launch too, with revision 5's credential: the delete matches only the
            // exact refresh token the server rejected, so a newer sign-in is never taken by it.
            crate::session::server_ended(owner, epoch, stored.refresh_token.clone());
            (false, false)
        }
        GateDecision::Closed {
            clear_secrets: false,
        } => (false, false),
    }
}

fn stored_from(secrets: AccountSecrets, last_refresh_unix: i64) -> StoredAccount {
    let mut stored = StoredAccount::from_secrets(&secrets);
    stored.last_refresh_unix = last_refresh_unix;
    stored
}

fn gate_value() -> HostResult {
    let resolved = resolve();
    let server_url = resolved
        .as_ref()
        .map(|item| item.url.clone())
        .unwrap_or_default();
    let detail = resolved
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    let stored = load_stored().ok().flatten();
    let unlocked = crate::account_gate::is_unlocked();
    let offline = unlocked && stored.as_ref().is_some_and(|item| item.offline);
    let email = stored.as_ref().and_then(|item| item.email.as_deref());
    Ok(HostObject::new()
        .field("unlocked", unlocked)
        .field("offline", offline)
        .field("server_url", server_url)
        .field("email", optional_text(email))
        .field("detail", detail)
        .into())
}

pub(crate) fn spawn_account(
    name: &str,
    work: impl FnOnce() -> HostResult + Send + 'static,
) -> Result<std::pin::Pin<Box<dyn std::future::Future<Output = HostResult> + Send>>, HostError> {
    let (sender, receiver) = futures::channel::oneshot::channel();
    // The account file lives next to the database, so this thread must know the database's name as the host
    // thread does: the in-process name is per thread, and a new thread is given it here. The device
    // folder override rides with it, the same discipline — tests read their own scratch install.
    let database = bir_core::db::capture_database_path_override();
    let device = bir_core::platform::capture_device_dir_override();
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let _ = sender.send(database.run(move || device.run(work)));
        })
        .map_err(|error| HostError::new(error.to_string()))?;
    Ok(Box::pin(async move {
        receiver
            .await
            .unwrap_or_else(|_| Err(HostError::new("The Buwiz account thread stopped.")))
    }))
}

pub(crate) fn resolve() -> Result<cloud_account::ResolvedServer, HostError> {
    cloud_account::configured_server().map_err(host_error)
}

fn forget_saved_server_url() {
    if let Err(error) = with_account_database(|database| {
        database
            .delete_setting(OBSOLETE_SERVER_URL_SETTING)
            .map_err(|error| HostError::new(error.to_string()))
    }) {
        tracing::warn!("could not drop the saved Buwiz server address: {error}");
    }
}

/// accounts-and-lock W3: whether the signed-in account (the host's saved sign-in, never a word from the page) signs
/// in on this device for the first time, recorded on a true (`bir_core::welcome`). No account signed in: false, and
/// nothing recorded.
pub(crate) fn welcome_first_time() -> HostResult {
    crate::account_gate::ensure_unlocked()?;
    drop_old_welcomed_list();
    let Some(account) = signed_in_account_key() else {
        return Ok(HostValue::Bool(false));
    };
    bir_core::welcome::first_time(&bir_core::rooms::device_store_here(), &account)
        .map(HostValue::Bool)
        .map_err(host_error)
}

/// The signed-in account's key (`bir_core::welcome::account_key`), as `cloud_account_status` sees the sign-in.
fn signed_in_account_key() -> Option<String> {
    let resolved = resolve().ok()?;
    let stored = load_stored().ok().flatten()?;
    let status = cloud_account::status_from(&resolved, Some(&stored.to_secrets()));
    if !status.signed_in {
        return None;
    }
    bir_core::welcome::account_key(status.user_id.as_deref(), status.email.as_deref())
}

/// #417's list of welcomed accounts held their user ids and addresses: it is dropped from `device_settings` and from
/// the open database's settings (where it was before A1), at launch and at each welcome check. At most one more
/// welcome.
pub(crate) fn drop_old_welcomed_list() {
    if let Err(error) = bir_core::welcome::drop_old_list(&bir_core::rooms::device_store_here()) {
        tracing::warn!("could not drop the old welcomed list from the device store: {error}");
    }
    if let Err(error) = with_account_database(|database| {
        database
            .delete_setting(bir_core::welcome::OLD_WELCOMED_SETTING)
            .map_err(|error| HostError::new(error.to_string()))
    }) {
        tracing::warn!("could not drop the old welcomed list from the database: {error}");
    }
}

fn rejected_origin_text(runtime_name: &str, compiled: Option<&str>) -> String {
    bir_core::config::var(runtime_name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            compiled
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

/// The account site's address for this build, as the screens open it (Delete account). Empty when it does not resolve.
fn account_url_text() -> String {
    bir_core::cloud_account::configured_account()
        .map(|account| account.url)
        .unwrap_or_default()
}

fn status_value(status: &AccountStatus) -> HostValue {
    HostObject::new()
        .field("server_url", status.server_url.clone())
        .field("server_url_source", status.server_url_source.clone())
        .field("account_url", account_url_text())
        .field("signed_in", status.signed_in)
        .field("email", optional_text(status.email.as_deref()))
        .field("user_id", optional_text(status.user_id.as_deref()))
        .field("offline", status.offline)
        .field("practice", status.practice)
        .field("detail", status.detail.clone())
        .into()
}

fn optional_text(value: Option<&str>) -> HostValue {
    value
        .filter(|text| !text.is_empty())
        .map(|text| HostValue::Str(text.to_owned()))
        .unwrap_or(HostValue::Null)
}

fn host_error(error: impl std::fmt::Display) -> HostError {
    HostError::new(error.to_string())
}

/// Server origin and the user access token, when this computer is signed in.
/// The device-filing poll uses it. Tokens are not logged.
#[cfg(feature = "agent")]
pub(crate) fn device_poll_auth() -> Option<(String, String)> {
    let stored = load_stored().ok().flatten()?;
    if stored.offline || stored.access_token.is_empty() || stored.server_url.is_empty() {
        return None;
    }
    Some((stored.server_url, stored.access_token))
}

pub(crate) fn load_stored() -> Result<Option<StoredAccount>, HostError> {
    cloud_account::load_stored_account().map_err(host_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test fixtures write through the door directly; production's clears are the machine's.
    fn clear_stored() {
        bir_core::account_session::clear_stored();
    }

    /// Tests seed the stored sign-in through the one path — the guard test allows no raw call
    /// in production code, so the fixtures live in here.
    pub(crate) fn save_stored(stored: &StoredAccount) -> Result<(), HostError> {
        bir_core::account_session::save_stored(stored).map_err(host_error)
    }
    use crate::bir_module::{use_ephemeral_database, with_account_database};

    /// The in-process database name is per thread; the account file lives next to the database, so a thread
    /// started for an account call works under the name of the thread that started it.
    #[test]
    fn an_account_thread_works_under_the_database_name_of_the_thread_that_started_it() {
        let folder = tempfile::tempdir().expect("scratch folder");
        let named = folder.path().join("bir_data.db");
        bir_core::db::set_database_path_override(Some(named.clone()));
        let seen = spawn_account("buwiz-account-test", || {
            Ok(HostValue::Str(
                bir_core::db::app_database_path().display().to_string(),
            ))
        })
        .map(futures::executor::block_on);
        bir_core::db::set_database_path_override(None);
        match seen {
            Ok(Ok(HostValue::Str(path))) => assert_eq!(path, named.display().to_string()),
            _ => panic!("the account thread did not answer"),
        }
    }

    #[test]
    fn the_page_cannot_set_a_server_url_and_a_saved_one_is_ignored() {
        assert!(
            !DECLARATIONS.contains("cloud_account_set_server_url"),
            "the host must not offer a server-url setter"
        );
        assert!(DECLARATIONS.contains("cloud_account_status"));
        use_ephemeral_database();
        with_account_database(|database| {
            database
                .set_setting(OBSOLETE_SERVER_URL_SETTING, "https://saved.example.test")
                .map_err(|error| HostError::new(error.to_string()))?;
            let stored = database
                .get_setting(OBSOLETE_SERVER_URL_SETTING)
                .map_err(|error| HostError::new(error.to_string()))?;
            assert_eq!(stored.as_deref(), Some("https://saved.example.test"));
            Ok(())
        })
        .unwrap();
        let resolved = resolve().expect("the build's server resolves");
        assert_ne!(
            resolved.url, "https://saved.example.test",
            "buwiz_server_url is not read"
        );
    }

    /// F10: session_opened never runs the claim on the calling thread — it returns while the
    /// room work is still blocked, and the work finishes on the spawned thread.
    #[test]
    fn session_opened_returns_while_the_room_work_is_still_running() {
        use_ephemeral_database();
        let release = crate::room_session::block_next_open_session_for_test();
        let opened = std::thread::spawn(session_opened);
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(
            opened.is_finished(),
            "session_opened is still blocked in the room work: the claim runs on the calling thread"
        );
        let _ = release.send(());
        let _ = opened.join();
        crate::account_gate::set_unlocked(false);
    }

    /// Startup deletes `buwiz_server_url`. The child uses a scratch database
    /// (`BIR_DATABASE_PATH`), `EBIR_TEST_ENV=1`, and `BIR_SFTP_DRY_RUN=1`.
    #[test]
    fn prepare_launch_deletes_a_saved_server_url() {
        const SCRATCH: &str = "BIR_SHELL_FORGET_SERVER_URL_SCRATCH";
        if std::env::var_os(SCRATCH).is_some() {
            delete_saved_server_url_on_startup();
            return;
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = std::env::temp_dir().join(format!(
            "bir-shell-forget-server-url-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&scratch).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "cloud_account::tests::prepare_launch_deletes_a_saved_server_url",
                "--nocapture",
            ])
            .current_dir(&scratch)
            .env(SCRATCH, &scratch)
            .env("BIR_DATABASE_PATH", scratch.join("bir_data.db"))
            .env("EBIR_TEST_ENV", "1")
            .env("BIR_AUTH_BYPASS", "1")
            .env("BIR_SFTP_DRY_RUN", "1")
            .env_remove("BUWIZ_SERVER_URL")
            .env_remove("BUWIZ_ACCOUNT_URL")
            .status()
            .unwrap();
        let _ = std::fs::remove_dir_all(&scratch);
        assert!(status.success(), "startup did not drop buwiz_server_url");
    }

    mod sign_out {
        use super::*;
        use bir_core::cloud_account::{HttpRequest, HttpResponse};
        use std::path::PathBuf;
        use std::sync::mpsc;
        use std::time::Duration;

        const SERVER: &str = "http://127.0.0.1:9";
        const WAIT: Duration = Duration::from_secs(10);

        /// What was still on this device when the logout reached the fake server, and what the request carried.
        struct Seen {
            url: String,
            bearer: Option<String>,
            file_left: bool,
            sign_in_left: bool,
        }

        /// The server's logout endpoint. It notes what is left on this device the moment the request arrives,
        /// then, when `release` is given, hangs until the test lets it go (or drops the sender), and answers.
        struct Logout {
            file: PathBuf,
            seen: mpsc::Sender<Seen>,
            release: Option<mpsc::Receiver<()>>,
            answer: fn() -> Result<HttpResponse, CloudError>,
        }

        impl cloud_account::Http for Logout {
            fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
                let _ = self.seen.send(Seen {
                    url: request.url.clone(),
                    bearer: request.bearer.clone(),
                    file_left: self.file.exists(),
                    sign_in_left: cloud_account::load_stored_account()
                        .ok()
                        .flatten()
                        .is_some(),
                });
                if let Some(release) = &self.release {
                    let _ = release.recv();
                }
                (self.answer)()
            }
        }

        fn answered() -> Result<HttpResponse, CloudError> {
            Ok(HttpResponse {
                status: 200,
                body: "{}".into(),
            })
        }

        fn refused() -> Result<HttpResponse, CloudError> {
            Ok(HttpResponse {
                status: 500,
                body: r#"{"error":"server_error"}"#.into(),
            })
        }

        fn unreachable() -> Result<HttpResponse, CloudError> {
            Err(CloudError::Network("connection refused".into()))
        }

        /// A scratch folder with a saved sign-in for [`SERVER`] (keychain off, so the private file is the record).
        fn signed_in<R>(body: impl FnOnce(PathBuf) -> R) -> R {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("account.json");
            bir_core::db::set_database_path_override(Some(dir.path().join("bir_data.db")));
            let result = temp_env::with_vars(
                [
                    ("BUWIZ_NO_KEYCHAIN", Some("1")),
                    ("BUWIZ_SERVER_URL", Some(SERVER)),
                    (
                        bir_core::email_sign_in::ENV_PENDING_FILE,
                        Some(dir.path().join("pending.json").to_str().unwrap()),
                    ),
                    (
                        cloud_account::ENV_ACCOUNT_FILE,
                        Some(file.to_str().unwrap()),
                    ),
                ],
                || {
                    save_stored(&stored("old-access", "old-refresh")).unwrap();
                    assert!(signed_in_now(), "the scratch sign-in did not take");
                    body(file.clone())
                },
            );
            bir_core::db::set_database_path_override(None);
            result
        }

        fn stored(access: &str, refresh: &str) -> StoredAccount {
            StoredAccount {
                server_url: SERVER.into(),
                access_token: access.into(),
                refresh_token: refresh.into(),
                access_expires_at_unix: i64::MAX / 2,
                user_id: "u1".into(),
                email: Some("filer@example.test".into()),
                offline: false,
                last_refresh_unix: 1_000,
                practice: false,
            }
        }

        fn signed_in_now() -> bool {
            signed_in_in(status_now())
        }

        fn signed_in_in(status: HostResult) -> bool {
            crate::bir_module::to_json(&status.expect("a status"))["signed_in"] == true
        }

        /// Runs the sign-out as the host does, on a thread of its own under this thread's database name, so a
        /// sign-out that waits on the server shows as a timeout here instead of a hung test.
        fn sign_out_on_a_thread(http: Logout) -> mpsc::Receiver<HostResult> {
            let (sender, receiver) = mpsc::channel();
            let database = bir_core::db::capture_database_path_override();
            std::thread::spawn(move || {
                let _ = sender.send(database.run(|| sign_out_with(Box::new(http))));
            });
            receiver
        }

        fn logout(
            file: &std::path::Path,
            answer: fn() -> Result<HttpResponse, CloudError>,
        ) -> (Logout, mpsc::Receiver<Seen>) {
            let (seen, heard) = mpsc::channel();
            let fake = Logout {
                file: file.to_path_buf(),
                seen,
                release: None,
                answer,
            };
            (fake, heard)
        }

        /// (a), (d), (e): when the logout request goes out, the saved sign-in is already gone; the request
        /// carries the old access token; the status, and what a relaunch reads, say signed out.
        #[test]
        fn the_saved_sign_in_is_gone_before_the_logout_request_and_the_request_carries_the_old_token()
         {
            signed_in(|file| {
                let (fake, heard) = logout(&file, answered);
                let status = sign_out_on_a_thread(fake)
                    .recv_timeout(WAIT)
                    .expect("sign-out returned");
                let seen = heard.recv_timeout(WAIT).expect("the logout was sent");
                assert!(
                    !seen.file_left,
                    "the account file was still there at the logout request"
                );
                assert!(
                    !seen.sign_in_left,
                    "the sign-in was still readable at the logout request"
                );
                assert_eq!(seen.url, format!("{SERVER}/api/auth/logout"));
                assert_eq!(seen.bearer.as_deref(), Some("old-access"));
                assert!(!signed_in_in(status), "sign-out answered signed in");
                assert!(!file.exists());
                assert!(!signed_in_now(), "a relaunch would read signed in");
            });
        }

        /// X3: the real sign_out_with runs on its own spawned thread (the account thread in the app), and the
        /// scan a screen made on the engine thread is forgotten there too — the scan state is process-wide.
        #[test]
        fn sign_out_on_the_account_thread_forgets_the_engines_scan() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|file| {
                let legacy_folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../bir-core/tests/legacy-ebirforms");
                crate::legacy_import_module::scan_folder(&legacy_folder).unwrap();
                assert!(crate::legacy_import_module::holds_scan());
                let (fake, _heard) = logout(&file, answered);
                sign_out_on_a_thread(fake)
                    .recv_timeout(WAIT)
                    .expect("sign-out returned");
                assert!(
                    !crate::legacy_import_module::holds_scan(),
                    "the scan survived the sign-out: it is per-thread, and the account thread cleared only its own"
                );
            });
        }

        /// (b), (d): a logout the server never answers does not hold the sign-out: it returns signed out, with
        /// nothing left on this device, while the request still hangs.
        #[test]
        fn sign_out_returns_signed_out_while_the_logout_request_still_hangs() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|file| {
                let (mut fake, heard) = logout(&file, answered);
                let (release, held) = mpsc::channel();
                fake.release = Some(held);
                let done = sign_out_on_a_thread(fake);
                let seen = heard.recv_timeout(WAIT).expect("the logout was sent");
                let status = done
                    .recv_timeout(WAIT)
                    .expect("sign-out returned while the logout request still hangs");
                assert!(
                    !seen.file_left,
                    "the account file was still there at the logout request"
                );
                assert_eq!(seen.bearer.as_deref(), Some("old-access"));
                assert!(!signed_in_in(status), "sign-out answered signed in");
                assert!(!file.exists(), "the account file outlived a hanging logout");
                assert!(!signed_in_now(), "a relaunch would read signed in");
                release.send(()).unwrap();
            });
        }

        /// (c), (d): a server that refuses the logout, or cannot be reached, changes nothing on this device: the
        /// sign-in was gone before the request, and stays gone.
        #[test]
        fn a_refused_or_unreachable_logout_leaves_the_device_signed_out() {
            let _turn = crate::open_room::room_test_turn();
            for answer in [refused, unreachable] {
                signed_in(|file| {
                    let (fake, heard) = logout(&file, answer);
                    let status = sign_out_on_a_thread(fake)
                        .recv_timeout(WAIT)
                        .expect("sign-out returned");
                    let seen = heard.recv_timeout(WAIT).expect("the logout was sent");
                    assert!(
                        !seen.file_left,
                        "the account file was still there at the logout request"
                    );
                    assert_eq!(seen.bearer.as_deref(), Some("old-access"));
                    assert!(!signed_in_in(status), "sign-out answered signed in");
                    assert!(!file.exists());
                    assert!(!signed_in_now(), "a relaunch would read signed in");
                });
            }
        }

        /// A new sign-in saved while the old logout still hangs is not held up by it (the account-session lock is
        /// free), and survives it: the old logout carries only the old token.
        #[test]
        fn a_new_sign_in_while_the_old_logout_hangs_is_saved_and_kept() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|file| {
                let (mut fake, heard) = logout(&file, answered);
                let (release, held) = mpsc::channel();
                fake.release = Some(held);
                let done = sign_out_on_a_thread(fake);
                let seen = heard.recv_timeout(WAIT).expect("the logout was sent");
                let (saved, was_saved) = mpsc::channel();
                let database = bir_core::db::capture_database_path_override();
                std::thread::spawn(move || {
                    let _ = saved.send(database.run(|| {
                        let _one = bir_core::account_session::lock();
                        save_stored(&stored("new-access", "new-refresh"))
                    }));
                });
                was_saved
                    .recv_timeout(WAIT)
                    .expect("a new sign-in waited on the old logout")
                    .unwrap();
                release.send(()).unwrap();
                done.recv_timeout(WAIT).expect("sign-out returned").unwrap();
                assert_eq!(seen.bearer.as_deref(), Some("old-access"));
                let kept = load_stored().unwrap().expect("the new sign-in is kept");
                assert_eq!(kept.access_token, "new-access");
                assert!(signed_in_now());
                clear_stored();
            });
        }

        /// A sign-in this device cannot remove (its folder and file read-only, so the delete, the second delete
        /// and the overwrite all fail) is an error the page shows, never "signed out". The gate is locked and
        /// the logout still goes out.
        #[cfg(unix)]
        #[test]
        fn a_sign_in_that_cannot_be_removed_is_an_error_and_the_gate_stays_locked() {
            let _turn = crate::open_room::room_test_turn();
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &std::path::Path, mode: u32| {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
            };
            signed_in(|file| {
                let folder = file.parent().unwrap().to_path_buf();
                mode(&file, 0o400);
                mode(&folder, 0o555);
                crate::account_gate::set_unlocked(true);
                let (fake, heard) = logout(&file, answered);
                let answer = sign_out_on_a_thread(fake)
                    .recv_timeout(WAIT)
                    .expect("sign-out returned");
                let unlocked = crate::account_gate::unlocked_flag();
                let seen = heard.recv_timeout(WAIT);
                mode(&folder, 0o755);
                mode(&file, 0o600);
                let error =
                    answer.expect_err("sign-out said signed out with the sign-in still saved");
                assert!(
                    error.to_string().contains("Sign-out did not finish"),
                    "{error}"
                );
                assert!(!unlocked, "the gate was left open");
                let seen = seen.expect("the logout was sent");
                assert_eq!(seen.bearer.as_deref(), Some("old-access"));
                clear_stored();
            });
        }

        /// The token endpoint and `/me` of the account server: `/oauth/token` takes `old-refresh` once and
        /// rotates it, as the real server does; `/me` knows only the rotated access token.
        #[derive(Default)]
        struct Rotating {
            refreshed: bool,
        }

        impl cloud_account::Http for Rotating {
            fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
                let answer = |status: u16, body: &str| {
                    Ok(HttpResponse {
                        status,
                        body: body.into(),
                    })
                };
                match request.url.trim_start_matches(SERVER) {
                    "/oauth/token" if !self.refreshed && request.body.contains("old-refresh") => {
                        self.refreshed = true;
                        answer(
                            200,
                            r#"{"access_token":"rotated-access","refresh_token":"rotated-refresh","expires_in":900}"#,
                        )
                    }
                    "/oauth/token" => answer(400, r#"{"error":"invalid_grant"}"#),
                    "/me" if request.bearer.as_deref() == Some("rotated-access") => {
                        answer(200, r#"{"user_id":"u1","email":"filer@example.test"}"#)
                    }
                    _ => answer(401, r#"{"error":"invalid_token"}"#),
                }
            }
        }

        /// The launch restore of a sign-in whose access token has expired: the refresh rotates the pair, the
        /// rotated pair is saved, and the gate opens.
        #[test]
        fn a_launch_restore_with_an_expired_token_saves_the_rotated_pair_and_opens_the_gate() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let mut expired = stored("old-access", "old-refresh");
                expired.access_expires_at_unix = 0;
                save_stored(&expired).unwrap();
                let mut server = Rotating::default();
                assert_eq!(restore_saved_session_with(&mut server), (true, false));
                assert!(server.refreshed, "the restore did not refresh");
                let saved = load_stored().unwrap().expect("still signed in");
                assert_eq!(
                    (saved.access_token.as_str(), saved.refresh_token.as_str()),
                    ("rotated-access", "rotated-refresh")
                );
                assert!(signed_in_now());
                clear_stored();
            });
        }
    }

    fn delete_saved_server_url_on_startup() {
        with_account_database(|database| {
            database
                .set_setting(OBSOLETE_SERVER_URL_SETTING, "https://saved.example.test")
                .map_err(|error| HostError::new(error.to_string()))?;
            Ok(())
        })
        .unwrap();
        let resolved = resolve().expect("the build's server resolves");
        assert_ne!(resolved.url, "https://saved.example.test");
        assert_ne!(resolved.source, cloud_account::UrlSource::Environment);
        if option_env!("BUWIZ_SERVER_URL")
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        {
            assert_eq!(
                resolved.url,
                cloud_account::profile_server_url(cloud_account::active_build_profile())
            );
            assert_eq!(resolved.source, cloud_account::UrlSource::Default);
        }
        prepare_launch();
        let left = with_account_database(|database| {
            database
                .get_setting(OBSOLETE_SERVER_URL_SETTING)
                .map_err(|error| HostError::new(error.to_string()))
        })
        .unwrap();
        assert_eq!(left, None);
    }
}
