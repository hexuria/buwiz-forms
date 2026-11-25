//! Host verbs for the Buwiz account in Settings.
//!
//! The page sees account status only. Refresh tokens stay in the OS keychain
//! when Secret Service (or the macOS keychain) accepts them, and otherwise in
//! a mode-0600 file beside the database — the same fallback the database key
//! uses when the keychain is missing. The page cannot read either store.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    practice: boolean;
    detail: string;
}
export interface CloudSignInStart {
    user_code: string;
    verification_uri: string;
    short_uri: string;
    browser_opened: boolean;
}
export interface CloudSignInFinish {
    outcome: "approved" | "cancelled" | "expired";
    status: CloudAccountStatus;
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
export function cloud_account_begin_sign_in(): Promise<CloudSignInStart>;
export function cloud_account_begin_sign_up(): Promise<CloudSignInStart>;
export function cloud_account_finish_sign_in(): Promise<CloudSignInFinish>;
export function cloud_account_cancel_sign_in(): null;
export function cloud_account_reopen_sign_in(): Promise<boolean>;
export function cloud_account_refresh(): Promise<CloudAccountStatus>;
export function cloud_account_sign_out(): Promise<CloudAccountStatus>;
export function cloud_account_session_opened(): null;
export function cloud_account_session_closed(): null;
"#;

/// The device sign-in that is waiting for its approval: the code, the page opened for it, and the switch that stops its
/// wait.
struct Pending {
    challenge: DeviceChallenge,
    page: String,
    stop: Arc<AtomicBool>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// The longest a Cancel waits for the poll to notice it.
const STOP_SLICE: Duration = Duration::from_millis(200);

pub fn register(module: HostModule) -> HostModule {
    module
        .function("cloud_account_status", |_| status_now())
        .function("cloud_account_gate", |_| gate_value())
        .async_function("cloud_account_begin_sign_in", begin_sign_in)
        .async_function("cloud_account_begin_sign_up", begin_sign_up)
        .async_function("cloud_account_finish_sign_in", finish_sign_in)
        .function("cloud_account_cancel_sign_in", |_| cancel_sign_in())
        .async_function("cloud_account_reopen_sign_in", reopen_sign_in)
        .async_function("cloud_account_refresh", refresh_account)
        .async_function("cloud_account_sign_out", sign_out)
        .function("cloud_account_session_opened", |_| session_opened())
        .function("cloud_account_session_closed", |_| session_closed())
}

/// Pause filing, then open the UI only when a saved session still qualifies.
/// Call this before the menu bar is built and before the database worker starts.
pub fn prepare_launch() {
    bir_core::account_session::on_session_ended(session_ended_by_server);
    bir_core::background_cron::set_submission_paused(true);
    forget_saved_server_url();
    if crate::account_gate::auth_bypass_enabled() {
        bir_core::transport::set_practice_account(false);
        crate::account_gate::set_unlocked(true);
        return;
    }
    let (unlocked, practice) = restore_saved_session();
    // Set before the gate opens, so no send can start ahead of it.
    bir_core::transport::set_practice_account(unlocked && practice);
    crate::account_gate::set_unlocked(unlocked);
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
        // The page itself is never shown: only the short address (for typing) and the code are.
        let user_code = challenge.user_code.clone();
        let verification_uri = challenge.verification_uri.clone();
        set_pending(challenge, url.clone());
        let browser_opened = cloud_account::open_browser(&url).is_ok();
        Ok(HostObject::new()
            .field("user_code", user_code)
            .field("short_uri", cloud_account::short_address(&verification_uri))
            .field("verification_uri", verification_uri)
            .field("browser_opened", browser_opened)
            .into())
    })
}

/// A new sign-in replaces the one waiting, whose wait stops.
fn set_pending(challenge: DeviceChallenge, page: String) {
    let previous = pending_lock().replace(Pending {
        challenge,
        page,
        stop: Arc::new(AtomicBool::new(false)),
    });
    if let Some(previous) = previous {
        previous.stop.store(true, Ordering::SeqCst);
    }
}

fn finish_sign_in(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    finish_pending(BlockingHttp::new())
}

/// Waits for the pending sign-in's approval with `http`. The pending record stays while it waits (Open again reopens
/// its page; Cancel stops it), and goes when the wait ends, however it ends. Cancelled and expired are answers, not
/// errors.
fn finish_pending<H: cloud_account::Http + Send + 'static>(
    mut http: H,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<H>, HostError> {
    let (challenge, stop) = {
        let pending = pending_lock();
        let pending = pending
            .as_ref()
            .ok_or_else(|| HostError::new("Sign-in has not started. Press Sign in again."))?;
        (pending.challenge.clone(), Arc::clone(&pending.stop))
    };
    spawn_account("buwiz-account-finish", move || {
        let stopped = || stop.load(Ordering::SeqCst);
        let mut now = cloud_account::unix_now;
        let mut sleep = |delay: Duration| {
            cloud_account::sleep_unless_stopped(
                delay,
                STOP_SLICE,
                &mut || stopped(),
                &mut |piece| std::thread::sleep(piece),
            )
        };
        let polled = cloud_account::poll_until_stopped(
            &mut http,
            &challenge,
            &mut now,
            &mut sleep,
            &mut || stopped(),
        );
        forget_pending(&challenge.device_code);
        let tokens = match polled {
            Ok(_) if stopped() => return finish_value("cancelled"),
            Ok(tokens) => tokens,
            Err(CloudError::Cancelled) => return finish_value("cancelled"),
            Err(CloudError::TimedOut) => return finish_value("expired"),
            Err(error) => return Err(host_error(error)),
        };
        if tokens.refresh_token.is_empty() {
            return Err(HostError::new(
                "The server did not return a refresh token. Sign in again.",
            ));
        }
        let (user_id, email, practice) =
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
            practice,
        };
        {
            let _one = bir_core::account_session::lock();
            bir_core::transport::set_practice_account(practice);
            save_stored(&stored_from(secrets, now))?;
        }
        // A new sign-in registers a new device key with the relay (the old one belonged to the old sign-in).
        bir_core::relay::forget_device();
        finish_value("approved")
    })
}

fn finish_value(outcome: &str) -> HostResult {
    Ok(HostObject::new()
        .field("outcome", outcome)
        .field("status", status_now()?)
        .into())
}

/// Cancel: the waiting sign-in stops (its poll notices within [`STOP_SLICE`]) and is forgotten, so Sign in starts a new
/// one. Nothing waiting: nothing to do.
fn cancel_sign_in() -> HostResult {
    clear_pending();
    Ok(HostValue::Null)
}

/// Open the sign-in page again: the same page, for the same code, in the browser. Answers whether it opened.
fn reopen_sign_in(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let page = pending_page()?;
    spawn_account("buwiz-account-reopen", move || {
        Ok(HostValue::Bool(cloud_account::open_browser(&page).is_ok()))
    })
}

fn pending_page() -> Result<String, HostError> {
    pending_lock()
        .as_ref()
        .map(|pending| pending.page.clone())
        .ok_or_else(|| HostError::new("No sign-in is waiting. Press Sign in again."))
}

/// Forgets the pending sign-in if it is still the one with `device_code` (a newer one stays).
fn forget_pending(device_code: &str) {
    let mut pending = pending_lock();
    if pending
        .as_ref()
        .is_some_and(|pending| pending.challenge.device_code == device_code)
    {
        *pending = None;
    }
}

fn refresh_account(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    spawn_account("buwiz-account-refresh", || {
        let _one = bir_core::account_session::lock();
        let Some(stored) = load_stored()? else {
            return status_now();
        };
        let resolved = resolve()?;
        if stored.server_url != resolved.url {
            clear_stored();
            return status_now();
        }
        let mut http = BlockingHttp::new();
        let now = cloud_account::unix_now();
        match cloud_account::refresh_session(&mut http, &stored.to_secrets(), now)
            .map_err(host_error)?
        {
            SessionUpdate::Online(secrets) => {
                bir_core::transport::set_practice_account(secrets.practice);
                save_stored(&stored_from(secrets, now))?;
            }
            SessionUpdate::Offline(secrets) => {
                bir_core::transport::set_practice_account(secrets.practice);
                save_stored(&stored_from(secrets, stored.last_refresh_unix))?;
            }
            SessionUpdate::SignedOut => {
                clear_stored();
                crate::account_gate::set_unlocked(false);
                bir_core::transport::set_practice_account(false);
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
        {
            let _one = bir_core::account_session::lock();
            if let Some(stored) = load_stored()? {
                let mut http = BlockingHttp::new();
                cloud_account::revoke_session(&mut http, &stored.to_secrets());
            }
            clear_stored();
        }
        crate::account_gate::set_unlocked(false);
        bir_core::transport::set_practice_account(false);
        bir_core::relay::forget_device();
        status_now()
    })
}

fn session_opened() -> HostResult {
    // The page opens the gate after a sign-in; take the flag from the saved
    // account. An unreadable store leaves the value finish_sign_in set.
    if let Ok(Some(stored)) = load_stored() {
        bir_core::transport::set_practice_account(stored.practice);
    }
    crate::account_gate::set_unlocked(true);
    crate::bir_module::open_database_at_launch();
    crate::hooks::hooks().start_updater();
    crate::hooks::hooks().refresh_menu();
    Ok(HostValue::Null)
}

fn session_closed() -> HostResult {
    crate::account_gate::set_unlocked(false);
    bir_core::transport::set_practice_account(false);
    crate::hooks::hooks().refresh_menu();
    Ok(HostValue::Null)
}

/// The sign-in ended outside a user action (a rejected refresh, a removed
/// device): `bir_core::account_session` has cleared the saved account.
fn session_ended_by_server() {
    crate::account_gate::set_unlocked(false);
    bir_core::transport::set_practice_account(false);
    crate::hooks::hooks().refresh_menu();
}

/// Whether the gate opens, and whether the account is a practice account.
/// Offline, the saved value stands.
fn restore_saved_session() -> (bool, bool) {
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
    let now = cloud_account::unix_now();
    let mut http = BlockingHttp::new();
    let previous_refresh = stored.last_refresh_unix;
    let mut practice = stored.practice;
    let outcome = match cloud_account::refresh_session(&mut http, &stored.to_secrets(), now) {
        Ok(SessionUpdate::Online(secrets)) => {
            practice = secrets.practice;
            if save_stored(&stored_from(secrets, now)).is_err() {
                return (false, false);
            }
            RefreshOutcome::Online
        }
        Ok(SessionUpdate::Offline(secrets)) => {
            practice = secrets.practice;
            let _ = save_stored(&stored_from(secrets, previous_refresh));
            RefreshOutcome::Offline
        }
        Ok(SessionUpdate::SignedOut) => RefreshOutcome::Revoked,
        Err(CloudError::InvalidGrant) => RefreshOutcome::Revoked,
        Err(_) => RefreshOutcome::Offline,
    };
    match cloud_account::gate_after_refresh(outcome) {
        GateDecision::Open { .. } => (true, practice),
        GateDecision::Closed {
            clear_secrets: true,
        } => {
            clear_stored();
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

fn spawn_account(
    name: &str,
    work: impl FnOnce() -> HostResult + Send + 'static,
) -> Result<std::pin::Pin<Box<dyn std::future::Future<Output = HostResult> + Send>>, HostError> {
    let (sender, receiver) = futures::channel::oneshot::channel();
    // The account file lives next to the database, so this thread must know the database's name as the host
    // thread does: the in-process name is per thread, and a new thread is given it here.
    let database = bir_core::db::capture_database_path_override();
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let _ = sender.send(database.run(work));
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

fn pending_lock() -> std::sync::MutexGuard<'static, Option<Pending>> {
    PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Stops and forgets the waiting sign-in, if any.
fn clear_pending() {
    if let Some(pending) = pending_lock().take() {
        pending.stop.store(true, Ordering::SeqCst);
    }
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

fn load_stored() -> Result<Option<StoredAccount>, HostError> {
    cloud_account::load_stored_account().map_err(host_error)
}

fn save_stored(stored: &StoredAccount) -> Result<(), HostError> {
    cloud_account::save_stored_account(stored).map_err(host_error)
}

fn clear_stored() {
    cloud_account::clear_stored_account();
}

#[cfg(test)]
mod tests {
    use super::*;
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

    /// A server that never approves (no network: the test's own answer).
    struct NeverApproved;
    impl cloud_account::Http for NeverApproved {
        fn exchange(
            &mut self,
            _: &cloud_account::HttpRequest,
        ) -> Result<cloud_account::HttpResponse, CloudError> {
            Ok(cloud_account::HttpResponse {
                status: 400,
                body: r#"{"error":"authorization_pending"}"#.into(),
            })
        }
    }

    fn waiting_challenge(device_code: &str) -> DeviceChallenge {
        DeviceChallenge {
            server_url: "http://127.0.0.1:9".into(),
            device_code: device_code.into(),
            user_code: "WXYZ-2345".into(),
            verification_uri: "http://127.0.0.1:9/device".into(),
            verification_uri_complete: "http://127.0.0.1:9/device?user_code=WXYZ-2345".into(),
            interval: Duration::from_secs(5),
            expires_at_unix: cloud_account::unix_now() + 900,
        }
    }

    fn outcome(value: HostValue) -> String {
        let json = crate::bir_module::to_json(&value);
        json["outcome"].as_str().unwrap_or_default().to_owned()
    }

    /// Cancel, Open again and a new sign-in, one after another (they share the one pending sign-in, so one test).
    #[test]
    fn cancel_stops_the_wait_promptly_and_a_new_sign_in_starts_clean() {
        use_ephemeral_database();
        clear_pending();
        // Nothing waiting: Open again says so, Cancel is harmless, finishing is refused.
        let error = pending_page().unwrap_err();
        assert_eq!(
            error.message(),
            "No sign-in is waiting. Press Sign in again."
        );
        assert!(matches!(cancel_sign_in(), Ok(HostValue::Null)));
        assert!(finish_pending(NeverApproved).is_err());

        for device_code in ["first", "second"] {
            set_pending(
                waiting_challenge(device_code),
                "http://127.0.0.1:9/login".into(),
            );
            assert_eq!(pending_page().unwrap(), "http://127.0.0.1:9/login");
            let waiting = finish_pending(NeverApproved).expect("the wait starts");
            // Let the poll reach its 5-second sleep, then cancel.
            std::thread::sleep(Duration::from_millis(300));
            let cancelled_at = std::time::Instant::now();
            cancel_sign_in().unwrap();
            let answer =
                futures::executor::block_on(waiting).expect("cancelled is an answer, not an error");
            assert!(
                cancelled_at.elapsed() < Duration::from_millis(600),
                "the wait noticed the Cancel late: {:?}",
                cancelled_at.elapsed()
            );
            assert_eq!(outcome(answer), "cancelled");
            assert!(pending_lock().is_none(), "Cancel forgets the code");
            assert!(pending_page().is_err());
        }

        // A second Sign in while one waits stops the first wait; the second stays pending.
        set_pending(waiting_challenge("old"), "http://127.0.0.1:9/old".into());
        let old = finish_pending(NeverApproved).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        set_pending(waiting_challenge("new"), "http://127.0.0.1:9/new".into());
        assert_eq!(
            outcome(futures::executor::block_on(old).unwrap()),
            "cancelled"
        );
        assert_eq!(pending_page().unwrap(), "http://127.0.0.1:9/new");
        clear_pending();
    }

    #[test]
    fn the_host_answer_names_no_full_page_address() {
        assert!(!DECLARATIONS.contains("verification_uri_complete"));
        assert!(DECLARATIONS.contains("short_uri"));
        assert!(DECLARATIONS.contains("cloud_account_cancel_sign_in"));
        assert!(DECLARATIONS.contains("cloud_account_reopen_sign_in"));
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
