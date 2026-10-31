//! Host verbs for the Buwiz account in Settings.
//!
//! The page sees account status only. Refresh tokens stay in the OS keychain
//! when Secret Service (or the macOS keychain) accepts them, and otherwise in
//! a mode-0600 file beside the database — the same fallback the database key
//! uses when the keychain is missing. The page cannot read either store.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bir_core::cloud_account::{
    self, AccountBrowser, AccountSecrets, AccountStatus, BlockingHttp, CloudError, DeviceChallenge,
    ENV_SERVER_URL, GateDecision, RefreshOutcome, SessionUpdate, StoredAccount,
};
use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};

use crate::bir_module::with_account_database;

/// Old Settings field. Startup deletes it. Nothing reads it.
const OBSOLETE_SERVER_URL_SETTING: &str = "buwiz_server_url";

pub const DECLARATIONS: &str = r#"
export interface CloudAccountStatus {
    server_url: string;
    server_url_source: string;
    signed_in: boolean;
    email: string | null;
    user_id: string | null;
    offline: boolean;
    detail: string;
}
export interface CloudSignInStart {
    user_code: string;
    verification_uri: string;
    verification_uri_complete: string;
    browser_opened: boolean;
}
export interface CloudAccountGate {
    unlocked: boolean;
    /** The saved sign-in is still being checked after launch; see `cloud_account_restored`. */
    restoring: boolean;
    offline: boolean;
    server_url: string;
    email: string | null;
    detail: string;
}
export function cloud_account_status(): CloudAccountStatus;
export function cloud_account_gate(): CloudAccountGate;
export function cloud_account_begin_sign_in(): Promise<CloudSignInStart>;
export function cloud_account_begin_sign_up(): Promise<CloudSignInStart>;
export function cloud_account_finish_sign_in(): Promise<CloudAccountStatus>;
export function cloud_account_refresh(): Promise<CloudAccountStatus>;
export function cloud_account_sign_out(): Promise<CloudAccountStatus>;
export function cloud_account_session_opened(): null;
export function cloud_account_session_closed(): null;
/** Resolves once the launch check of the saved sign-in has finished (at once when none is running). */
export function cloud_account_restored(): Promise<null>;
"#;

static PENDING: Mutex<Option<DeviceChallenge>> = Mutex::new(None);

/// The saved account as this process last read or wrote it, and when. The
/// gate, Settings and the device-filing poll ask for it on the UI thread,
/// and the read behind it is the OS keychain plus a file. Every save and
/// clear here writes through; a read older than [`STORED_TTL`] goes back to
/// the store, so a token another BIR process rotated is picked up. Refresh,
/// restore and sign-out always read the store itself.
static STORED: Mutex<Option<(Instant, Option<StoredAccount>)>> = Mutex::new(None);
const STORED_TTL: Duration = Duration::from_secs(300);

/// The launch check of the saved sign-in is running on its own thread.
static RESTORING: AtomicBool = AtomicBool::new(false);
/// `cloud_account_restored` calls waiting for that check.
static RESTORE_WAITERS: Mutex<Vec<futures::channel::oneshot::Sender<()>>> = Mutex::new(Vec::new());

/// Why the saved sign-in is gone after Buwiz rejected its refresh.
const SIGN_IN_ENDED: &str = "Sign in again: Buwiz ended this computer's sign-in (its refresh was \
rejected, which happens when two BIR apps refresh the same sign-in).";

pub fn register(module: HostModule) -> HostModule {
    module
        .function("cloud_account_status", |_| status_now())
        .function("cloud_account_gate", |_| gate_value())
        .async_function("cloud_account_begin_sign_in", begin_sign_in)
        .async_function("cloud_account_begin_sign_up", begin_sign_up)
        .async_function("cloud_account_finish_sign_in", finish_sign_in)
        .async_function("cloud_account_refresh", refresh_account)
        .async_function("cloud_account_sign_out", sign_out)
        .function("cloud_account_session_opened", |_| session_opened())
        .function("cloud_account_session_closed", |_| session_closed())
        .async_function("cloud_account_restored", restored)
}

/// Pause filing, then open the UI only when a saved session still qualifies.
/// Call this before the menu bar is built and before the database worker starts.
/// Windowless (`bir-mini-app`): the check runs here, on the calling thread.
#[cfg(any(feature = "agent", test))]
pub(crate) fn prepare_launch() {
    if begin_launch() {
        crate::account_gate::set_unlocked(restore_saved_session());
        RESTORING.store(false, Ordering::SeqCst);
    }
}

/// Pause filing and lock the gate. True when a saved session still has to
/// be checked: the window host runs that with [`restore_in_background`] so
/// the window opens first; the test bypass has already unlocked. From a
/// true answer on, the gate reports `restoring` (the page reads it while
/// the window opens, before the check has started).
pub(crate) fn begin_launch() -> bool {
    bir_core::background_cron::set_submission_paused(true);
    forget_saved_server_url();
    if crate::account_gate::auth_bypass_enabled() {
        crate::account_gate::set_unlocked(true);
        return false;
    }
    crate::account_gate::set_unlocked(false);
    RESTORING.store(true, Ordering::SeqCst);
    true
}

/// Check the saved session (a token refresh, up to the refresh lock's wait
/// plus the request timeout) on its own thread, then apply the answer on the
/// UI thread exactly as a launch that had waited for it: open the gate, the
/// database (which starts the submission worker), the updater and the menu.
pub(crate) fn restore_in_background(cx: &mut gpui_kit::App) {
    let (sender, receiver) = futures::channel::oneshot::channel::<bool>();
    let spawned = std::thread::Builder::new()
        .name("buwiz-account-restore".into())
        .spawn(move || {
            let _ = sender.send(restore_saved_session());
        });
    if let Err(error) = spawned {
        tracing::error!("saved sign-in not checked: {error}");
        finish_restore(false, cx);
        return;
    }
    cx.spawn(async move |cx| {
        let unlocked = receiver.await.unwrap_or(false);
        cx.update(|cx| finish_restore(unlocked, cx));
    })
    .detach();
}

fn finish_restore(unlocked: bool, cx: &mut gpui_kit::App) {
    if unlocked && !crate::account_gate::is_unlocked() {
        crate::account_gate::set_unlocked(true);
        crate::bir_module::open_database_at_launch();
        crate::updater::start();
        crate::app_menu::refresh(cx);
    }
    RESTORING.store(false, Ordering::SeqCst);
    for waiter in restore_waiters().drain(..) {
        let _ = waiter.send(());
    }
}

fn restore_waiters() -> std::sync::MutexGuard<'static, Vec<futures::channel::oneshot::Sender<()>>> {
    RESTORE_WAITERS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// `cloud_account_restored`: settles when the launch check has been applied.
fn restored(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let (sender, receiver) = futures::channel::oneshot::channel::<()>();
    {
        let mut waiters = restore_waiters();
        if RESTORING.load(Ordering::SeqCst) {
            waiters.push(sender);
        } else {
            let _ = sender.send(());
        }
    }
    Ok(async move {
        let _ = receiver.await;
        Ok(HostValue::Null)
    })
}

fn status_now() -> HostResult {
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
                detail: error.to_string(),
            }));
        }
    };
    let stored = load_stored_cached().ok().flatten();
    let secrets = stored
        .as_ref()
        .filter(|stored| stored.server_url == resolved.url)
        .map(StoredAccount::to_secrets);
    Ok(status_value(&cloud_account::status_from(
        &resolved,
        secrets.as_ref(),
    )))
}

fn begin_sign_in(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    begin_browser(AccountBrowser::SignIn)
}

fn begin_sign_up(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    begin_browser(AccountBrowser::CreateAccount)
}

fn begin_browser(
    entry: AccountBrowser,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    spawn_account("buwiz-account-start", move || {
        let resolved = resolve()?;
        let account = cloud_account::configured_account().map_err(host_error)?;
        let mut http = BlockingHttp::new();
        let challenge =
            cloud_account::start_device(&mut http, &resolved.url, cloud_account::unix_now())
                .map_err(host_error)?;
        let url = cloud_account::account_browser_url(&account.url, entry, &challenge.user_code);
        let browser_opened = cloud_account::open_browser(&url).is_ok();
        let started = HostObject::new()
            .field("user_code", challenge.user_code.clone())
            .field("verification_uri", challenge.verification_uri.clone())
            .field("verification_uri_complete", url)
            .field("browser_opened", browser_opened)
            .into();
        *pending_lock() = Some(challenge);
        Ok(started)
    })
}

fn finish_sign_in(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let challenge = pending_lock()
        .take()
        .ok_or_else(|| HostError::new("Sign-in has not started. Press Sign in again."))?;
    spawn_account("buwiz-account-finish", move || {
        let mut http = BlockingHttp::new();
        let mut now = cloud_account::unix_now;
        let mut sleep = |delay: Duration| std::thread::sleep(delay);
        let tokens = cloud_account::poll_until(&mut http, &challenge, &mut now, &mut sleep)
            .map_err(host_error)?;
        if tokens.refresh_token.is_empty() {
            return Err(HostError::new(
                "The server did not return a refresh token. Sign in again.",
            ));
        }
        let (user_id, email) =
            cloud_account::load_profile(&mut http, &challenge.server_url, &tokens.access_token)
                .map_err(host_error)?;
        let now = cloud_account::unix_now();
        let secrets = AccountSecrets {
            server_url: challenge.server_url.clone(),
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            access_expires_at_unix: now.saturating_add(tokens.expires_in),
            user_id,
            email,
            offline: false,
        };
        save_stored(&stored_from(secrets, now))?;
        status_now()
    })
}

fn refresh_account(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    spawn_account("buwiz-account-refresh", || {
        // Held across the refresh, and the account read under it: another
        // BIR process may have just rotated the refresh token.
        let _lock = cloud_account::lock_refresh().map_err(host_error)?;
        let Some(stored) = load_stored()? else {
            return status_now();
        };
        let resolved = resolve()?;
        if stored.server_url != resolved.url {
            clear_stored();
            return status_now();
        }
        let mut http = BlockingHttp::with_timeout(cloud_account::REFRESH_TIMEOUT);
        let now = cloud_account::unix_now();
        match cloud_account::refresh_session(&mut http, &stored.to_secrets(), now)
            .map_err(host_error)?
        {
            SessionUpdate::Online(secrets) => {
                save_stored(&stored_from(secrets, now))?;
            }
            SessionUpdate::Offline(secrets) => {
                save_stored(&stored_from(secrets, stored.last_refresh_unix))?;
            }
            SessionUpdate::SignedOut => {
                clear_stored();
                cloud_account::record_sign_in_ended(SIGN_IN_ENDED);
                crate::account_gate::set_unlocked(false);
            }
        }
        status_now()
    })
}

fn sign_out(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    spawn_account("buwiz-account-sign-out", || {
        clear_pending();
        if let Some(stored) = load_stored()? {
            let mut http = BlockingHttp::new();
            cloud_account::revoke_session(&mut http, &stored.to_secrets());
        }
        clear_stored();
        crate::account_gate::set_unlocked(false);
        status_now()
    })
}

fn session_opened() -> HostResult {
    crate::account_gate::set_unlocked(true);
    crate::bir_module::open_database_at_launch();
    crate::updater::start();
    let _ = gpui_shell::with_current_app(crate::app_menu::refresh);
    Ok(HostValue::Null)
}

fn session_closed() -> HostResult {
    crate::account_gate::set_unlocked(false);
    let _ = gpui_shell::with_current_app(crate::app_menu::refresh);
    Ok(HostValue::Null)
}

fn restore_saved_session() -> bool {
    // Another BIR process refreshing the same account holds this lock; read
    // the account after it, so a token it rotated is never used again.
    let lock = cloud_account::lock_refresh();
    let Ok(Some(stored)) = load_stored() else {
        return false;
    };
    if lock.is_err() {
        // Still busy elsewhere: keep the saved session without refreshing.
        return matches!(
            cloud_account::gate_after_refresh(RefreshOutcome::Offline),
            GateDecision::Open { .. }
        );
    }
    let Ok(resolved) = resolve() else {
        return false;
    };
    if stored.server_url != resolved.url {
        return false;
    }
    let now = cloud_account::unix_now();
    let mut http = BlockingHttp::with_timeout(cloud_account::REFRESH_TIMEOUT);
    let previous_refresh = stored.last_refresh_unix;
    let outcome = match cloud_account::refresh_session(&mut http, &stored.to_secrets(), now) {
        Ok(SessionUpdate::Online(secrets)) => {
            if save_stored(&stored_from(secrets, now)).is_err() {
                return false;
            }
            RefreshOutcome::Online
        }
        Ok(SessionUpdate::Offline(secrets)) => {
            let _ = save_stored(&stored_from(secrets, previous_refresh));
            RefreshOutcome::Offline
        }
        Ok(SessionUpdate::SignedOut) => RefreshOutcome::Revoked,
        Err(CloudError::InvalidGrant) => RefreshOutcome::Revoked,
        Err(_) => RefreshOutcome::Offline,
    };
    match cloud_account::gate_after_refresh(outcome) {
        GateDecision::Open { .. } => true,
        GateDecision::Closed {
            clear_secrets: true,
        } => {
            clear_stored();
            cloud_account::record_sign_in_ended(SIGN_IN_ENDED);
            false
        }
        GateDecision::Closed {
            clear_secrets: false,
        } => false,
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
    let stored = load_stored_cached().ok().flatten();
    let unlocked = crate::account_gate::is_unlocked();
    let offline = unlocked && stored.as_ref().is_some_and(|item| item.offline);
    let email = stored.as_ref().and_then(|item| item.email.as_deref());
    Ok(HostObject::new()
        .field("unlocked", unlocked)
        .field("restoring", RESTORING.load(Ordering::SeqCst))
        .field("offline", offline)
        .field("server_url", server_url)
        .field("email", optional_text(email))
        .field("detail", detail)
        .into())
}

fn spawn_account(
    name: &str,
    work: impl FnOnce() -> HostResult + Send + 'static,
) -> Result<std::pin::Pin<Box<dyn std::future::Future<Output = HostResult> + Send>>, HostError> {
    let (sender, receiver) = futures::channel::oneshot::channel();
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let _ = sender.send(work());
        })
        .map_err(|error| HostError::new(error.to_string()))?;
    Ok(Box::pin(async move {
        receiver
            .await
            .unwrap_or_else(|_| Err(HostError::new("The Buwiz account thread stopped.")))
    }))
}

fn resolve() -> Result<cloud_account::ResolvedServer, HostError> {
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

fn rejected_origin_text(runtime_name: &str, compiled: Option<&str>) -> String {
    std::env::var(runtime_name)
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

fn status_value(status: &AccountStatus) -> HostValue {
    HostObject::new()
        .field("server_url", status.server_url.clone())
        .field("server_url_source", status.server_url_source.clone())
        .field("signed_in", status.signed_in)
        .field("email", optional_text(status.email.as_deref()))
        .field("user_id", optional_text(status.user_id.as_deref()))
        .field("offline", status.offline)
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

fn pending_lock() -> std::sync::MutexGuard<'static, Option<DeviceChallenge>> {
    PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn clear_pending() {
    *pending_lock() = None;
}

/// Server origin and the user access token, when this computer is signed in.
/// The device-filing poll uses it. Tokens are not logged.
#[cfg(feature = "agent")]
pub(crate) fn device_poll_auth() -> Option<(String, String)> {
    let stored = load_stored_cached().ok().flatten()?;
    if stored.offline || stored.access_token.is_empty() || stored.server_url.is_empty() {
        return None;
    }
    Some((stored.server_url, stored.access_token))
}

/// The saved account read from the keychain and file, which also refreshes
/// [`STORED`]. Use under the refresh lock and wherever a rotated token matters.
fn load_stored() -> Result<Option<StoredAccount>, HostError> {
    let stored = cloud_account::load_stored_account().map_err(host_error)?;
    remember_stored(stored.clone());
    Ok(stored)
}

/// [`STORED`] when it is younger than [`STORED_TTL`], else [`load_stored`].
fn load_stored_cached() -> Result<Option<StoredAccount>, HostError> {
    if let Some((read_at, stored)) = stored_cache().as_ref()
        && read_at.elapsed() < STORED_TTL
    {
        return Ok(stored.clone());
    }
    load_stored()
}

fn save_stored(stored: &StoredAccount) -> Result<(), HostError> {
    cloud_account::save_stored_account(stored).map_err(host_error)?;
    remember_stored(Some(stored.clone()));
    Ok(())
}

fn clear_stored() {
    cloud_account::clear_stored_account();
    remember_stored(None);
}

fn remember_stored(stored: Option<StoredAccount>) {
    *stored_cache() = Some((Instant::now(), stored));
}

fn stored_cache() -> std::sync::MutexGuard<'static, Option<(Instant, Option<StoredAccount>)>> {
    STORED.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bir_module::{use_ephemeral_database, with_account_database};

    /// buwiz-server on 127.0.0.1 only, BIR configured with `localhost`
    /// (which macOS resolves to ::1 first): the account's client reaches it.
    #[test]
    fn the_account_client_reaches_localhost_on_ipv4_only() {
        use bir_core::cloud_account::{BlockingHttp, Http, HttpRequest};
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 11\r\nconnection: close\r\n\r\n{\"ok\":true}")
                .unwrap();
        });
        let response = BlockingHttp::with_timeout(std::time::Duration::from_secs(5))
            .exchange(&HttpRequest {
                method: "GET",
                url: format!("http://localhost:{port}/me"),
                content_type: "application/json",
                bearer: None,
                body: String::new(),
            })
            .unwrap_or_else(|_| panic!("http://localhost:{port} did not reach 127.0.0.1"));
        assert_eq!(response.status, 200);
        assert_eq!(response.body, r#"{"ok":true}"#);
        server.join().unwrap();
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
