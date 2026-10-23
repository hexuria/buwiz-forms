//! Host verbs for the Buwiz account in Settings.
//!
//! The page sees account status only. Refresh tokens stay in the OS keychain
//! when Secret Service (or the macOS keychain) accepts them, and otherwise in
//! a mode-0600 file beside the database — the same fallback the database key
//! uses when the keychain is missing. The page cannot read either store.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use bir_core::cloud_account::{
    self, AccountBrowser, AccountSecrets, AccountStatus, BlockingHttp, CloudError, DeviceChallenge,
    ENV_SERVER_URL, GateDecision, RefreshOutcome, SessionUpdate, StoredAccount,
};
use bir_core::db;
use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};

use crate::bir_module::with_account_database;

const SETTING_KEY: &str = "buwiz_server_url";
const KEYRING_SERVICE: &str = "com.buwiz.bir";
const KEYRING_ACCOUNT: &str = "desktop-refresh";

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
    offline: boolean;
    server_url: string;
    email: string | null;
    detail: string;
}
export function cloud_account_status(): CloudAccountStatus;
export function cloud_account_gate(): CloudAccountGate;
export function cloud_account_set_server_url(url: string): CloudAccountStatus;
export function cloud_account_begin_sign_in(): Promise<CloudSignInStart>;
export function cloud_account_begin_sign_up(): Promise<CloudSignInStart>;
export function cloud_account_finish_sign_in(): Promise<CloudAccountStatus>;
export function cloud_account_refresh(): Promise<CloudAccountStatus>;
export function cloud_account_sign_out(): Promise<CloudAccountStatus>;
export function cloud_account_session_opened(): null;
export function cloud_account_session_closed(): null;
"#;

static PENDING: Mutex<Option<DeviceChallenge>> = Mutex::new(None);

pub fn register(module: HostModule) -> HostModule {
    module
        .function("cloud_account_status", |_| status_now())
        .function("cloud_account_gate", |_| gate_value())
        .function("cloud_account_set_server_url", set_server_url)
        .async_function("cloud_account_begin_sign_in", begin_sign_in)
        .async_function("cloud_account_begin_sign_up", begin_sign_up)
        .async_function("cloud_account_finish_sign_in", finish_sign_in)
        .async_function("cloud_account_refresh", refresh_account)
        .async_function("cloud_account_sign_out", sign_out)
        .function("cloud_account_session_opened", |_| session_opened())
        .function("cloud_account_session_closed", |_| session_closed())
}

/// Pause filing, then open the UI only when a saved session still qualifies.
/// Call this before the menu bar is built and before the database worker starts.
pub(crate) fn prepare_launch() {
    bir_core::background_cron::set_submission_paused(true);
    if crate::account_gate::auth_bypass_enabled() {
        crate::account_gate::set_unlocked(true);
        return;
    }
    crate::account_gate::set_unlocked(restore_saved_session());
}

fn status_now() -> HostResult {
    let resolved = match resolve() {
        Ok(resolved) => resolved,
        Err(error) => {
            return Ok(status_value(&AccountStatus {
                server_url: saved_server_url().unwrap_or_default(),
                server_url_source: "invalid".into(),
                signed_in: false,
                email: None,
                user_id: None,
                offline: false,
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

fn set_server_url(arguments: &HostArguments) -> HostResult {
    let raw = arguments.string(0)?.trim().to_owned();
    let normalized = cloud_account::normalize_server_url(&raw).map_err(host_error)?;
    let previous = saved_server_url();
    with_account_database(|database| {
        database
            .set_setting(SETTING_KEY, &normalized)
            .map_err(host_error)?;
        Ok(())
    })?;
    if previous.as_deref() != Some(normalized.as_str()) {
        clear_pending();
        clear_stored();
    }
    status_now()
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
        let mut http = BlockingHttp::new();
        let challenge =
            cloud_account::start_device(&mut http, &resolved.url, cloud_account::unix_now())
                .map_err(host_error)?;
        let url =
            cloud_account::account_browser_url(&challenge.server_url, entry, &challenge.user_code);
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
                save_stored(&stored_from(secrets, now))?;
            }
            SessionUpdate::Offline(secrets) => {
                save_stored(&stored_from(secrets, stored.last_refresh_unix))?;
            }
            SessionUpdate::SignedOut => {
                clear_stored();
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
    let Ok(Some(stored)) = load_stored() else {
        return false;
    };
    let Ok(resolved) = resolve() else {
        return false;
    };
    if stored.server_url != resolved.url {
        return false;
    }
    let now = cloud_account::unix_now();
    let mut http = BlockingHttp::new();
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
    match cloud_account::gate_after_refresh(previous_refresh, now, outcome) {
        GateDecision::Open { .. } => true,
        GateDecision::Closed {
            clear_secrets: true,
        } => {
            clear_stored();
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
    let settings = saved_server_url();
    let env_value = std::env::var(ENV_SERVER_URL).ok();
    cloud_account::resolve_server_url(settings.as_deref(), env_value.as_deref()).map_err(host_error)
}

fn saved_server_url() -> Option<String> {
    with_account_database(|database| {
        database
            .get_setting(SETTING_KEY)
            .map_err(|error| HostError::new(error.to_string()))
    })
    .ok()
    .flatten()
    .filter(|value| !value.trim().is_empty())
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

fn load_stored() -> Result<Option<StoredAccount>, HostError> {
    if let Some(raw) = keyring_load()
        && let Ok(stored) = StoredAccount::decode(&raw)
    {
        return Ok(Some(stored));
    }
    let path = secret_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        HostError::new(format!("could not read the saved Buwiz account ({error})"))
    })?;
    StoredAccount::decode(&raw).map(Some).map_err(host_error)
}

fn save_stored(stored: &StoredAccount) -> Result<(), HostError> {
    let raw = stored.encode().map_err(host_error)?;
    // The keychain is preferred. The private file is always written too: a
    // keyring build without a real backend reports success and then forgets
    // the secret when the process exits.
    let _ = keyring_save(&raw);
    write_secret_file(&raw)
}

fn clear_stored() {
    keyring_delete();
    if let Ok(path) = secret_path() {
        let _ = fs::remove_file(path);
    }
}

fn secret_path() -> Result<PathBuf, HostError> {
    if let Some(path) = std::env::var_os("BUWIZ_CLOUD_ACCOUNT_FILE") {
        return Ok(PathBuf::from(path));
    }
    let database = db::app_database_path();
    let directory = database
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Ok(directory.join("buwiz-cloud-account.json"))
}

fn write_secret_file(raw: &str) -> Result<(), HostError> {
    let path = secret_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| HostError::new(error.to_string()))?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .map_err(|error| HostError::new(format!("could not store the Buwiz account ({error})")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
    }
    file.write_all(raw.as_bytes())
        .map_err(|error| HostError::new(format!("could not store the Buwiz account ({error})")))?;
    Ok(())
}

fn keyring_entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
}

fn keyring_load() -> Option<String> {
    let entry = keyring_entry().ok()?;
    match entry.get_password() {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

fn keyring_save(raw: &str) -> bool {
    keyring_entry()
        .and_then(|entry| entry.set_password(raw))
        .is_ok()
}

fn keyring_delete() {
    if let Ok(entry) = keyring_entry() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => tracing::debug!("Buwiz keychain entry not deleted: {error}"),
        }
    }
}
