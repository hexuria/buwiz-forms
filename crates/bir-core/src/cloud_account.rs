//! Sign-in against buwiz-server for the desktop app.
//!
//! The server already issues first-party tokens through the device-code flow
//! (`POST /auth/device/start`, approval in the browser at `/login/device`,
//! `POST /auth/device/poll`) and refreshes them at `POST /oauth/token`.
//! This module speaks that contract. It does not file returns and it does
//! not read taxpayer records. A network failure leaves any saved session in
//! place so the app can keep preparing returns on this computer.
//!
//! The saved account is also what `bir-mcp` reads. There is one keychain
//! entry and one private file. `bir-mcp` does not keep a second sign-in.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};

/// Production API. A debug build uses [`DEV_SERVER_URL`] when neither the
/// compile-time nor the runtime `BUWIZ_SERVER_URL` is set. Not a secret.
pub const PRODUCTION_SERVER_URL: &str = "https://api.buwiz.com";
/// Browser origin for account pages in a release build. The API stays on
/// [`PRODUCTION_SERVER_URL`]. Not a secret.
pub const PRODUCTION_ACCOUNT_URL: &str = "https://account.buwiz.com";
/// API and account origin for a debug build when no URL was configured.
pub const DEV_SERVER_URL: &str = "http://localhost:3008";
pub const DESKTOP_CLIENT_ID: &str = "buwiz-desktop";
pub const ENV_SERVER_URL: &str = "BUWIZ_SERVER_URL";
pub const ENV_ACCOUNT_URL: &str = "BUWIZ_ACCOUNT_URL";
/// OS keychain service for the desktop refresh token. `bir-mcp` uses this
/// same entry.
pub const KEYRING_SERVICE: &str = "com.buwiz.bir";
/// OS keychain account for [`KEYRING_SERVICE`].
pub const KEYRING_ACCOUNT: &str = "desktop-refresh";
/// Overrides the private account file. The default sits next to the database.
pub const ENV_ACCOUNT_FILE: &str = "BUWIZ_CLOUD_ACCOUNT_FILE";
const ACCOUNT_FILE_NAME: &str = "buwiz-cloud-account.json";

const USER_AGENT: &str = "buwiz-desktop";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlSource {
    /// Process environment, for a dev or test run.
    Environment,
    /// `option_env!` baked in when this binary was compiled.
    Build,
    /// Debug or release fallback. No URL was configured.
    Default,
}

impl UrlSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "env",
            Self::Build => "build",
            Self::Default => "default",
        }
    }
}

/// Which fallback a binary uses when no URL was configured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildProfile {
    Debug,
    Release,
}

pub fn active_build_profile() -> BuildProfile {
    if cfg!(debug_assertions) {
        BuildProfile::Debug
    } else {
        BuildProfile::Release
    }
}

pub fn profile_server_url(profile: BuildProfile) -> &'static str {
    match profile {
        BuildProfile::Debug => DEV_SERVER_URL,
        BuildProfile::Release => PRODUCTION_SERVER_URL,
    }
}

pub fn profile_account_url(profile: BuildProfile) -> &'static str {
    match profile {
        BuildProfile::Debug => DEV_SERVER_URL,
        BuildProfile::Release => PRODUCTION_ACCOUNT_URL,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedServer {
    pub url: String,
    pub source: UrlSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerUrlError {
    Empty,
    UnsupportedScheme,
    HasCredentials,
    HasPath,
    HasQuery,
    Invalid,
}

impl std::fmt::Display for ServerUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "The Buwiz server address is empty."),
            Self::UnsupportedScheme => {
                write!(f, "The server address must start with https:// or http://.")
            }
            Self::HasCredentials => write!(
                f,
                "The server address cannot include a username or password."
            ),
            Self::HasPath => write!(f, "The server address is only the origin, with no path."),
            Self::HasQuery => write!(
                f,
                "The server address cannot include a query or a fragment."
            ),
            Self::Invalid => write!(f, "That server address is not a valid URL."),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloudError {
    Network(String),
    InvalidGrant,
    TimedOut,
    Server { description: String },
    Unexpected(String),
    Url(ServerUrlError),
}

impl std::fmt::Display for CloudError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(message) => write!(
                f,
                "The Buwiz server is unreachable ({message}). Filing on this computer still works."
            ),
            Self::InvalidGrant => write!(f, "This sign-in is no longer valid. Sign in again."),
            Self::TimedOut => write!(f, "Sign-in timed out before it was approved. Start again."),
            Self::Server { description } => write!(f, "{description}"),
            Self::Unexpected(message) => write!(f, "{message}"),
            Self::Url(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CloudError {}

#[derive(Clone, PartialEq, Eq)]
pub struct AccountSecrets {
    pub server_url: String,
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at_unix: i64,
    pub user_id: String,
    pub email: Option<String>,
    pub offline: bool,
}

impl std::fmt::Debug for AccountSecrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountSecrets")
            .field("server_url", &self.server_url)
            .field("user_id", &self.user_id)
            .field("email", &self.email)
            .field("offline", &self.offline)
            .field("access_expires_at_unix", &self.access_expires_at_unix)
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .finish()
    }
}

/// What Settings shows. Tokens never appear here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AccountStatus {
    pub server_url: String,
    pub server_url_source: String,
    pub signed_in: bool,
    pub email: Option<String>,
    pub user_id: Option<String>,
    pub offline: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceChallenge {
    pub server_url: String,
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub interval: Duration,
    pub expires_at_unix: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub content_type: &'static str,
    pub bearer: Option<String>,
    pub body: String,
}

pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

pub trait Http {
    fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError>;
}

pub enum SessionUpdate {
    Online(AccountSecrets),
    Offline(AccountSecrets),
    SignedOut,
}

/// What the shell should do with the filing UI after a refresh attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshOutcome {
    Online,
    Offline,
    Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateDecision {
    Open {
        offline: bool,
    },
    /// `clear_secrets` is set when the server rejected the refresh token.
    /// A network failure keeps the token so the next launch can try again.
    Closed {
        clear_secrets: bool,
    },
}

/// Which browser page the desktop opens for a device code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountBrowser {
    SignIn,
    /// The server's signup path. `/signup` redirects to the real form.
    CreateAccount,
}

/// Saved session. Serialized only into the private token store.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredAccount {
    pub server_url: String,
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at_unix: i64,
    pub user_id: String,
    pub email: Option<String>,
    #[serde(default)]
    pub offline: bool,
    /// Unix time of the last refresh the server accepted. `0` means this
    /// install has not recorded one. An unreachable server still opens the
    /// app; only sign-out or an explicit rejection closes it.
    #[serde(default)]
    pub last_refresh_unix: i64,
}

impl std::fmt::Debug for StoredAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_secrets().fmt(f)
    }
}

impl StoredAccount {
    pub fn to_secrets(&self) -> AccountSecrets {
        AccountSecrets {
            server_url: self.server_url.clone(),
            access_token: self.access_token.clone(),
            refresh_token: self.refresh_token.clone(),
            access_expires_at_unix: self.access_expires_at_unix,
            user_id: self.user_id.clone(),
            email: self.email.clone(),
            offline: self.offline,
        }
    }

    pub fn from_secrets(secrets: &AccountSecrets) -> Self {
        Self {
            server_url: secrets.server_url.clone(),
            access_token: secrets.access_token.clone(),
            refresh_token: secrets.refresh_token.clone(),
            access_expires_at_unix: secrets.access_expires_at_unix,
            user_id: secrets.user_id.clone(),
            email: secrets.email.clone(),
            offline: secrets.offline,
            last_refresh_unix: 0,
        }
    }

    pub fn encode(&self) -> Result<String, CloudError> {
        serde_json::to_string(self)
            .map_err(|_| CloudError::Unexpected("could not store the Buwiz account".into()))
    }

    pub fn decode(raw: &str) -> Result<Self, CloudError> {
        let stored: Self = serde_json::from_str(raw).map_err(|_| {
            CloudError::Unexpected("the saved Buwiz account could not be read".into())
        })?;
        if stored.access_token.is_empty()
            || stored.refresh_token.is_empty()
            || stored.server_url.is_empty()
        {
            return Err(CloudError::Unexpected(
                "the saved Buwiz account is incomplete".into(),
            ));
        }
        Ok(stored)
    }
}

/// What to do with a saved session after one refresh attempt.
///
/// An unreachable server stays open. The gate returns only when the person
/// signs out, or when the server is reachable and rejects the token.
pub fn gate_after_refresh(outcome: RefreshOutcome) -> GateDecision {
    match outcome {
        RefreshOutcome::Online => GateDecision::Open { offline: false },
        RefreshOutcome::Revoked => GateDecision::Closed {
            clear_secrets: true,
        },
        RefreshOutcome::Offline => GateDecision::Open { offline: true },
    }
}

/// Browser URL for this device code. `account_origin` is the account site
/// (`https://account.buwiz.com` in a release build), not necessarily the API.
/// `next` is the approval page, so signup and sign-in both land back on the
/// same device flow.
pub fn account_browser_url(account_origin: &str, entry: AccountBrowser, user_code: &str) -> String {
    let next = format!("/device?user_code={user_code}");
    let page = match entry {
        AccountBrowser::SignIn => "login",
        AccountBrowser::CreateAccount => "signup",
    };
    format!(
        "{}/{page}?next={}",
        account_origin.trim_end_matches('/'),
        urlencoding::encode(&next)
    )
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// API origin. `runtime` is the process environment (`BUWIZ_SERVER_URL`).
/// `compiled` is `option_env!("BUWIZ_SERVER_URL")`. A runtime value wins.
/// Neither is a secret, and neither is something the person at the keyboard types.
pub fn resolve_server_url(
    runtime: Option<&str>,
    compiled: Option<&str>,
    profile: BuildProfile,
) -> Result<ResolvedServer, ServerUrlError> {
    resolve_origin(runtime, compiled, profile_server_url(profile))
}

/// Account-site origin. Same precedence as [`resolve_server_url`], with
/// `BUWIZ_ACCOUNT_URL`. Release builds open `https://account.buwiz.com`.
pub fn resolve_account_url(
    runtime: Option<&str>,
    compiled: Option<&str>,
    profile: BuildProfile,
) -> Result<ResolvedServer, ServerUrlError> {
    resolve_origin(runtime, compiled, profile_account_url(profile))
}

/// The API origin for this process.
pub fn configured_server() -> Result<ResolvedServer, ServerUrlError> {
    let runtime = std::env::var(ENV_SERVER_URL).ok();
    resolve_server_url(
        runtime.as_deref(),
        option_env!("BUWIZ_SERVER_URL"),
        active_build_profile(),
    )
}

/// Where the browser opens login and signup.
pub fn configured_account() -> Result<ResolvedServer, ServerUrlError> {
    let runtime = std::env::var(ENV_ACCOUNT_URL).ok();
    resolve_account_url(
        runtime.as_deref(),
        option_env!("BUWIZ_ACCOUNT_URL"),
        active_build_profile(),
    )
}

fn resolve_origin(
    runtime: Option<&str>,
    compiled: Option<&str>,
    fallback: &str,
) -> Result<ResolvedServer, ServerUrlError> {
    if let Some(value) = nonempty(runtime) {
        return Ok(ResolvedServer {
            url: normalize_server_url(value)?,
            source: UrlSource::Environment,
        });
    }
    if let Some(value) = nonempty(compiled) {
        return Ok(ResolvedServer {
            url: normalize_server_url(value)?,
            source: UrlSource::Build,
        });
    }
    Ok(ResolvedServer {
        url: fallback.to_owned(),
        source: UrlSource::Default,
    })
}

pub fn normalize_server_url(raw: &str) -> Result<String, ServerUrlError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(ServerUrlError::Empty);
    }
    let url = reqwest::Url::parse(raw).map_err(|_| ServerUrlError::Invalid)?;
    match url.scheme() {
        "https" | "http" => {}
        _ => return Err(ServerUrlError::UnsupportedScheme),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ServerUrlError::HasCredentials);
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(ServerUrlError::HasQuery);
    }
    let path = url.path();
    if path != "/" && !path.is_empty() {
        return Err(ServerUrlError::HasPath);
    }
    let host = url
        .host_str()
        .ok_or(ServerUrlError::Invalid)?
        .trim_matches(|character| character == '[' || character == ']');
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    let mut out = format!("{}://{host}", url.scheme());
    if let Some(port) = url.port() {
        out.push(':');
        out.push_str(&port.to_string());
    }
    Ok(out)
}

pub fn status_from(resolved: &ResolvedServer, secrets: Option<&AccountSecrets>) -> AccountStatus {
    let Some(secrets) = secrets.filter(|secrets| secrets.server_url == resolved.url) else {
        return AccountStatus {
            server_url: resolved.url.clone(),
            server_url_source: resolved.source.as_str().to_owned(),
            signed_in: false,
            email: None,
            user_id: None,
            offline: false,
            detail: "Not signed in. Preparing and filing returns on this computer does not need an account.".into(),
        };
    };
    let who = secrets
        .email
        .clone()
        .filter(|email| !email.is_empty())
        .unwrap_or_else(|| secrets.user_id.clone());
    let detail = if secrets.offline {
        format!("Offline. Still signed in as {who}. Filing on this computer still works.")
    } else {
        format!("Signed in as {who}.")
    };
    AccountStatus {
        server_url: resolved.url.clone(),
        server_url_source: resolved.source.as_str().to_owned(),
        signed_in: true,
        email: secrets.email.clone(),
        user_id: Some(secrets.user_id.clone()),
        offline: secrets.offline,
        detail,
    }
}

pub fn start_device(
    http: &mut dyn Http,
    server_url: &str,
    now_unix: i64,
) -> Result<DeviceChallenge, CloudError> {
    let response = http.exchange(&HttpRequest {
        method: "POST",
        url: format!("{server_url}/auth/device/start"),
        content_type: "application/json",
        bearer: None,
        body: json!({ "client_id": DESKTOP_CLIENT_ID }).to_string(),
    })?;
    let value = expect_json(response.status, &response.body)?;
    if !(200..300).contains(&response.status) {
        return Err(error_from_json(response.status, &value));
    }
    let device_code = required_str(&value, "device_code")?;
    let user_code = required_str(&value, "user_code")?;
    let verification_uri = required_str(&value, "verification_uri")?;
    let complete = value
        .get("verification_uri_complete")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{verification_uri}?user_code={user_code}"));
    let expires_in = json_i64(&value, "expires_in").unwrap_or(900).max(1);
    let interval = json_i64(&value, "interval").unwrap_or(5).max(1);
    Ok(DeviceChallenge {
        server_url: server_url.to_owned(),
        device_code,
        user_code,
        verification_uri,
        verification_uri_complete: complete,
        interval: Duration::from_secs(interval as u64),
        expires_at_unix: now_unix.saturating_add(expires_in),
    })
}

enum Poll {
    Pending,
    SlowDown,
    Ready(TokenSet),
}

pub fn poll_until(
    http: &mut dyn Http,
    challenge: &DeviceChallenge,
    now: &mut dyn FnMut() -> i64,
    sleep: &mut dyn FnMut(Duration),
) -> Result<TokenSet, CloudError> {
    // 900s / 1s, with room for slow_down. A frozen clock still stops.
    for _ in 0..1000 {
        if now() >= challenge.expires_at_unix {
            return Err(CloudError::TimedOut);
        }
        match poll_device(http, challenge) {
            Ok(Poll::Ready(tokens)) => return Ok(tokens),
            Ok(Poll::Pending) => sleep(challenge.interval),
            Ok(Poll::SlowDown) => sleep(challenge.interval.saturating_mul(2)),
            Err(CloudError::Network(_)) => sleep(challenge.interval),
            Err(error) => return Err(error),
        }
    }
    Err(CloudError::TimedOut)
}

fn poll_device(http: &mut dyn Http, challenge: &DeviceChallenge) -> Result<Poll, CloudError> {
    let response = http.exchange(&HttpRequest {
        method: "POST",
        url: format!("{}/auth/device/poll", challenge.server_url),
        content_type: "application/json",
        bearer: None,
        body: json!({
            "client_id": DESKTOP_CLIENT_ID,
            "device_code": challenge.device_code,
        })
        .to_string(),
    })?;
    interpret_token_response(response.status, &response.body, true)
}

pub fn refresh_tokens(
    http: &mut dyn Http,
    server_url: &str,
    refresh_token: &str,
) -> Result<TokenSet, CloudError> {
    let body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        urlencoding::encode(refresh_token),
        DESKTOP_CLIENT_ID
    );
    let response = http.exchange(&HttpRequest {
        method: "POST",
        url: format!("{server_url}/oauth/token"),
        content_type: "application/x-www-form-urlencoded",
        bearer: None,
        body,
    })?;
    match interpret_token_response(response.status, &response.body, false)? {
        Poll::Ready(mut tokens) => {
            if tokens.refresh_token.is_empty() {
                tokens.refresh_token = refresh_token.to_owned();
            }
            Ok(tokens)
        }
        Poll::Pending | Poll::SlowDown => Err(CloudError::Unexpected(
            "the server has not finished refreshing the sign-in".into(),
        )),
    }
}

pub fn load_profile(
    http: &mut dyn Http,
    server_url: &str,
    access_token: &str,
) -> Result<(String, Option<String>), CloudError> {
    let response = http.exchange(&HttpRequest {
        method: "GET",
        url: format!("{server_url}/me"),
        content_type: "application/json",
        bearer: Some(access_token.to_owned()),
        body: String::new(),
    })?;
    if response.status == 401 || response.status == 403 {
        return Err(CloudError::InvalidGrant);
    }
    let value = expect_json(response.status, &response.body)?;
    if !(200..300).contains(&response.status) {
        return Err(error_from_json(response.status, &value));
    }
    let user_id = required_str(&value, "user_id")?;
    let email = value
        .get("email")
        .and_then(Value::as_str)
        .filter(|email| !email.is_empty())
        .map(str::to_owned);
    Ok((user_id, email))
}

/// Keep the access token fresh. A network failure keeps the previous tokens
/// and marks the session offline. An invalid refresh token signs the session out.
pub fn refresh_session(
    http: &mut dyn Http,
    secrets: &AccountSecrets,
    now_unix: i64,
) -> Result<SessionUpdate, CloudError> {
    let access_still_valid = secrets.access_expires_at_unix > now_unix.saturating_add(30);
    if access_still_valid {
        match load_profile(http, &secrets.server_url, &secrets.access_token) {
            Ok((user_id, email)) => {
                let mut next = secrets.clone();
                next.user_id = user_id;
                next.email = email;
                next.offline = false;
                return Ok(SessionUpdate::Online(next));
            }
            Err(CloudError::Network(message)) => {
                let mut next = secrets.clone();
                next.offline = true;
                let _ = message;
                return Ok(SessionUpdate::Offline(next));
            }
            Err(CloudError::InvalidGrant) => {}
            Err(error) => return Err(error),
        }
    }
    match refresh_tokens(http, &secrets.server_url, &secrets.refresh_token) {
        Ok(tokens) => {
            let (user_id, email) =
                match load_profile(http, &secrets.server_url, &tokens.access_token) {
                    Ok(profile) => profile,
                    Err(CloudError::Network(_)) => {
                        let mut next = secrets.clone();
                        next.access_token = tokens.access_token;
                        next.refresh_token = tokens.refresh_token;
                        next.access_expires_at_unix =
                            now_unix.saturating_add(tokens.expires_in.max(1));
                        next.offline = true;
                        return Ok(SessionUpdate::Offline(next));
                    }
                    Err(CloudError::InvalidGrant) => return Ok(SessionUpdate::SignedOut),
                    Err(error) => return Err(error),
                };
            Ok(SessionUpdate::Online(AccountSecrets {
                server_url: secrets.server_url.clone(),
                access_token: tokens.access_token,
                refresh_token: tokens.refresh_token,
                access_expires_at_unix: now_unix.saturating_add(tokens.expires_in.max(1)),
                user_id,
                email,
                offline: false,
            }))
        }
        Err(CloudError::Network(_)) => {
            let mut next = secrets.clone();
            next.offline = true;
            Ok(SessionUpdate::Offline(next))
        }
        Err(CloudError::InvalidGrant) => Ok(SessionUpdate::SignedOut),
        Err(error) => Err(error),
    }
}

/// Best-effort revocation. Local sign-out still proceeds when this fails.
pub fn revoke_session(http: &mut dyn Http, secrets: &AccountSecrets) {
    let _ = http.exchange(&HttpRequest {
        method: "POST",
        url: format!("{}/api/auth/logout", secrets.server_url),
        content_type: "application/json",
        bearer: Some(secrets.access_token.clone()),
        body: "{}".into(),
    });
}

pub fn open_browser(url: &str) -> Result<(), String> {
    open::that(url).map_err(|error| error.to_string())
}

/// Load the desktop account. The keychain wins when it holds a complete
/// record. Otherwise the private file is used. `Ok(None)` means signed out.
pub fn load_stored_account() -> Result<Option<StoredAccount>, CloudError> {
    if let Some(raw) = keyring_load()
        && let Ok(stored) = StoredAccount::decode(&raw)
    {
        return Ok(Some(stored));
    }
    let path = account_file_path();
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|error| {
        CloudError::Unexpected(format!("could not read the saved Buwiz account ({error})"))
    })?;
    StoredAccount::decode(&raw).map(Some)
}

/// Write the desktop account to the keychain and the private file.
/// The file is always written: a keyring build without a real backend can
/// report success and then forget the secret when the process exits.
pub fn save_stored_account(stored: &StoredAccount) -> Result<(), CloudError> {
    let raw = stored.encode()?;
    let _ = keyring_save(&raw);
    write_secret_file(&raw)?;
    // A saved account is a live sign-in: forget why the last one ended.
    let _ = std::fs::remove_file(ended_file_path());
    Ok(())
}

/// Remove the keychain entry and the private file.
pub fn clear_stored_account() {
    keyring_delete();
    let _ = std::fs::remove_file(account_file_path());
}

/// How long [`lock_refresh`] waits for another process's refresh.
const REFRESH_LOCK_WAIT: Duration = Duration::from_secs(15);

/// An exclusive hold on the saved account for one refresh, across every BIR
/// process on this computer (the app, `bir-mini-app`, `bir-mcp`). Buwiz
/// rotates refresh tokens and revokes the whole sign-in when one is used
/// twice, so two processes must never refresh the same token: take this,
/// re-read the saved account, and refresh only if it is still stale. Released
/// when dropped (the OS also releases it when the process exits).
pub struct RefreshLock {
    _file: std::fs::File,
}

/// Take [`RefreshLock`]: an exclusive lock on `buwiz-cloud-account.lock`
/// beside the account file, waiting up to 15 seconds for another process.
pub fn lock_refresh() -> Result<RefreshLock, CloudError> {
    let path = account_file_path().with_extension("lock");
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|error| {
            CloudError::Unexpected(format!("could not lock the Buwiz account ({error})"))
        })?;
    let deadline = std::time::Instant::now() + REFRESH_LOCK_WAIT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(RefreshLock { _file: file }),
            Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(CloudError::Unexpected(
                    "another BIR app on this computer is still refreshing the Buwiz sign-in; try again in a moment"
                        .into(),
                ));
            }
            Err(std::fs::TryLockError::Error(error)) => {
                return Err(CloudError::Unexpected(format!(
                    "could not lock the Buwiz account ({error})"
                )));
            }
        }
    }
}

/// Remember why the saved sign-in ended (Buwiz rejected its refresh), so
/// every BIR process can say so at once instead of retrying. Cleared when
/// an account is saved again.
pub fn record_sign_in_ended(reason: &str) {
    let path = ended_file_path();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, reason);
}

/// Why the last saved sign-in ended, when it was rejected and nobody has
/// signed in since.
pub fn sign_in_ended() -> Option<String> {
    std::fs::read_to_string(ended_file_path())
        .ok()
        .map(|reason| reason.trim().to_string())
        .filter(|reason| !reason.is_empty())
}

fn ended_file_path() -> PathBuf {
    account_file_path().with_extension("ended")
}

fn account_file_path() -> PathBuf {
    if let Some(path) = std::env::var_os(ENV_ACCOUNT_FILE) {
        return PathBuf::from(path);
    }
    let database = crate::db::app_database_path();
    let directory = database
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    directory.join(ACCOUNT_FILE_NAME)
}

fn write_secret_file(raw: &str) -> Result<(), CloudError> {
    let path = account_file_path();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            CloudError::Unexpected(format!("could not store the Buwiz account ({error})"))
        })?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .map_err(|error| {
            CloudError::Unexpected(format!("could not store the Buwiz account ({error})"))
        })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
    }
    file.write_all(raw.as_bytes()).map_err(|error| {
        CloudError::Unexpected(format!("could not store the Buwiz account ({error})"))
    })?;
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

pub struct BlockingHttp {
    client: reqwest::blocking::Client,
}

impl BlockingHttp {
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_secs(20))
    }

    /// A client that gives up after `timeout` overall (and a third of it, at
    /// most 3 seconds, to connect): what a token refresh uses, so a server
    /// that never answers costs seconds, not every check's 20.
    pub fn with_timeout(timeout: Duration) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .connect_timeout((timeout / 3).min(Duration::from_secs(3)))
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self { client }
    }
}

/// How long a token refresh may take before it counts as unreachable.
pub const REFRESH_TIMEOUT: Duration = Duration::from_secs(8);

impl Default for BlockingHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl Http for BlockingHttp {
    fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
        let method = reqwest::Method::from_bytes(request.method.as_bytes())
            .map_err(|_| CloudError::Unexpected("unsupported HTTP method".into()))?;
        let mut builder = self
            .client
            .request(method, &request.url)
            .header("accept", "application/json");
        if !request.body.is_empty() || request.method != "GET" {
            builder = builder.header("content-type", request.content_type);
        }
        if let Some(token) = &request.bearer {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        if !request.body.is_empty() {
            // Spin's current WASI HTTP host can trap while reading a streamed
            // request body. buwiz-server already accepts the same payload in
            // `x-buwiz-request-body` and then discards the stream. Send both.
            builder = builder
                .header(
                    "x-buwiz-request-body",
                    standard_b64(request.body.as_bytes()),
                )
                .header("x-buwiz-request-body-enc", "b64")
                .body(request.body.clone());
        }
        let response = builder
            .send()
            .map_err(|error| CloudError::Network(error.to_string()))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|error| CloudError::Network(error.to_string()))?;
        Ok(HttpResponse { status, body })
    }
}

fn interpret_token_response(
    status: u16,
    body: &str,
    allow_pending: bool,
) -> Result<Poll, CloudError> {
    let value = match serde_json::from_str::<Value>(body) {
        Ok(value) => value,
        // A plain-text rejection of the token itself.
        Err(_) if (400..600).contains(&status) && names_a_rejected_token(body) => {
            return Err(CloudError::InvalidGrant);
        }
        Err(_) if !(200..300).contains(&status) => {
            return Err(CloudError::Server {
                description: format!("the Buwiz server returned HTTP {status}"),
            });
        }
        Err(_) => {
            return Err(CloudError::Unexpected(
                "the server returned an unexpected sign-in response".into(),
            ));
        }
    };
    // OAuth's `{"error": "invalid_grant"}`, or buwiz-server's own
    // `{"error": {"code": "invalid_token", "message": ...}}`.
    let error = value
        .get("error")
        .and_then(|error| error.as_str().or_else(|| error.get("code").and_then(Value::as_str)))
        .unwrap_or("");
    if allow_pending && error == "authorization_pending" {
        return Ok(Poll::Pending);
    }
    if allow_pending && error == "slow_down" {
        return Ok(Poll::SlowDown);
    }
    if error == "invalid_grant" || error == "invalid_token" || status == 401 {
        return Err(CloudError::InvalidGrant);
    }
    if error == "expired_token" || error == "access_denied" {
        let description = value
            .get("error_description")
            .and_then(Value::as_str)
            .unwrap_or(error);
        return Err(CloudError::Server {
            description: description.to_owned(),
        });
    }
    if !error.is_empty() || !(200..300).contains(&status) {
        return Err(error_from_json(status, &value));
    }
    let access_token = required_str(&value, "access_token")?;
    let refresh_token = value
        .get("refresh_token")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let expires_in = json_i64(&value, "expires_in")
        .or_else(|| json_i64(&value, "expires_in_seconds"))
        .unwrap_or(900)
        .max(1);
    Ok(Poll::Ready(TokenSet {
        access_token,
        refresh_token,
        expires_in,
    }))
}

/// Whether a non-JSON error body says the token was rejected.
fn names_a_rejected_token(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    ["invalid_grant", "invalid_token", "token is invalid"]
        .iter()
        .any(|needle| body.contains(needle))
}

fn expect_json(status: u16, body: &str) -> Result<Value, CloudError> {
    serde_json::from_str(body).map_err(|_| {
        if (200..300).contains(&status) {
            CloudError::Unexpected("the server returned an unexpected sign-in response".into())
        } else {
            CloudError::Server {
                description: format!("the Buwiz server returned HTTP {status}"),
            }
        }
    })
}

fn error_from_json(status: u16, value: &Value) -> CloudError {
    let description = value
        .get("error_description")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("the Buwiz server returned HTTP {status}"));
    CloudError::Server { description }
}

fn required_str(value: &Value, key: &str) -> Result<String, CloudError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            CloudError::Unexpected(format!("the server left {key} out of the sign-in response"))
        })
}

fn json_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|item| {
        item.as_i64()
            .or_else(|| item.as_u64().map(|number| number as i64))
            .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
    })
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

/// Standard base64, no newlines. Matches buwiz-server's `b64` body header.
fn standard_b64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let (chunks, rest) = bytes.as_chunks::<3>();
    for chunk in chunks {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        out.push(TABLE[(n & 0x3f) as usize] as char);
    }
    if rest.len() == 1 {
        let n = u32::from(rest[0]) << 16;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest.len() == 2 {
        let n = (u32::from(rest[0]) << 16) | (u32::from(rest[1]) << 8);
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Scripted {
        steps: Vec<Result<HttpResponse, CloudError>>,
        seen: Vec<String>,
    }

    impl Http for Scripted {
        fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
            self.seen
                .push(format!("{} {}", request.method, request.url));
            if request
                .bearer
                .as_deref()
                .is_some_and(|token| token.contains("access-secret"))
            {
                assert!(
                    !request.url.contains("access-secret"),
                    "access tokens stay in the Authorization header"
                );
            }
            assert!(
                !self.seen.last().unwrap().contains("refresh-secret"),
                "refresh tokens are not written into the URL"
            );
            self.steps
                .pop()
                .unwrap_or_else(|| Err(CloudError::Unexpected("no scripted response left".into())))
        }
    }

    fn challenge() -> DeviceChallenge {
        DeviceChallenge {
            server_url: "http://localhost:3008".into(),
            device_code: "device-1".into(),
            user_code: "ABCD-EFGH".into(),
            verification_uri: "http://localhost:3008/login/device".into(),
            verification_uri_complete: "http://localhost:3008/login/device?user_code=ABCD-EFGH"
                .into(),
            interval: Duration::from_secs(5),
            expires_at_unix: 10_000,
        }
    }

    fn secrets() -> AccountSecrets {
        AccountSecrets {
            server_url: "http://localhost:3008".into(),
            access_token: "access-secret".into(),
            refresh_token: "refresh-secret".into(),
            access_expires_at_unix: 500,
            user_id: "user-1".into(),
            email: Some("filer@example.test".into()),
            offline: false,
        }
    }

    #[test]
    fn server_url_precedence_and_rejection() {
        let release = resolve_server_url(None, None, BuildProfile::Release).unwrap();
        assert_eq!(release.url, PRODUCTION_SERVER_URL);
        assert_eq!(release.source, UrlSource::Default);

        let debug = resolve_server_url(None, None, BuildProfile::Debug).unwrap();
        assert_eq!(debug.url, DEV_SERVER_URL);
        assert_eq!(debug.source, UrlSource::Default);

        let blanks = resolve_server_url(Some("  "), Some(""), BuildProfile::Release).unwrap();
        assert_eq!(blanks.url, PRODUCTION_SERVER_URL);
        assert_eq!(blanks.source, UrlSource::Default);

        let from_build = resolve_server_url(
            None,
            Some(" https://staging.example.test/ "),
            BuildProfile::Release,
        )
        .unwrap();
        assert_eq!(from_build.url, "https://staging.example.test");
        assert_eq!(from_build.source, UrlSource::Build);

        let from_env = resolve_server_url(
            Some(" http://localhost:3008/ "),
            Some("https://staging.example.test"),
            BuildProfile::Release,
        )
        .unwrap();
        assert_eq!(from_env.url, "http://localhost:3008");
        assert_eq!(from_env.source, UrlSource::Environment);

        assert_eq!(
            profile_server_url(BuildProfile::Release),
            PRODUCTION_SERVER_URL
        );
        assert_eq!(profile_server_url(BuildProfile::Debug), DEV_SERVER_URL);
        assert_eq!(
            normalize_server_url("http://[::1]:3008").unwrap(),
            "http://[::1]:3008"
        );
        assert!(matches!(
            normalize_server_url(""),
            Err(ServerUrlError::Empty)
        ));
        assert!(matches!(
            normalize_server_url("javascript:alert(1)"),
            Err(ServerUrlError::UnsupportedScheme)
        ));
        assert!(matches!(
            normalize_server_url("https://user:pass@api.buwiz.com"),
            Err(ServerUrlError::HasCredentials)
        ));
        assert!(matches!(
            normalize_server_url("https://api.buwiz.com/tax-profiles"),
            Err(ServerUrlError::HasPath)
        ));
        assert!(matches!(
            normalize_server_url("https://api.buwiz.com/?q=1"),
            Err(ServerUrlError::HasQuery)
        ));
        assert!(matches!(
            resolve_server_url(None, Some("javascript:alert(1)"), BuildProfile::Release),
            Err(ServerUrlError::UnsupportedScheme)
        ));
        assert!(matches!(
            resolve_server_url(
                Some("https://api.buwiz.com/tax-profiles"),
                Some("https://staging.example.test"),
                BuildProfile::Debug,
            ),
            Err(ServerUrlError::HasPath)
        ));
    }

    #[test]
    fn account_origin_is_separate_from_the_api_and_follows_the_same_precedence() {
        let release = resolve_account_url(None, None, BuildProfile::Release).unwrap();
        assert_eq!(release.url, PRODUCTION_ACCOUNT_URL);
        assert_eq!(release.source, UrlSource::Default);
        assert_ne!(release.url, PRODUCTION_SERVER_URL);

        let debug = resolve_account_url(None, None, BuildProfile::Debug).unwrap();
        assert_eq!(debug.url, DEV_SERVER_URL);
        assert_eq!(debug.source, UrlSource::Default);

        let from_build = resolve_account_url(
            None,
            Some("https://accounts.example.test"),
            BuildProfile::Release,
        )
        .unwrap();
        assert_eq!(from_build.url, "https://accounts.example.test");
        assert_eq!(from_build.source, UrlSource::Build);

        let from_env = resolve_account_url(
            Some(" http://127.0.0.1:3008/ "),
            Some("https://accounts.example.test"),
            BuildProfile::Release,
        )
        .unwrap();
        assert_eq!(from_env.url, "http://127.0.0.1:3008");
        assert_eq!(from_env.source, UrlSource::Environment);

        let sign_in = account_browser_url(&release.url, AccountBrowser::SignIn, "ABCD-EFGH");
        assert_eq!(
            sign_in,
            "https://account.buwiz.com/login?next=%2Fdevice%3Fuser_code%3DABCD-EFGH"
        );
        assert!(!sign_in.contains("api.buwiz.com"));
        assert!(matches!(
            resolve_account_url(Some("javascript:alert(1)"), None, BuildProfile::Release),
            Err(ServerUrlError::UnsupportedScheme)
        ));
    }

    #[test]
    fn configured_origins_follow_runtime_then_compile_time_then_profile() {
        let profile = active_build_profile();
        let server_runtime = std::env::var(ENV_SERVER_URL).ok();
        let server = configured_server().unwrap();
        assert_eq!(
            server,
            resolve_server_url(
                server_runtime.as_deref(),
                option_env!("BUWIZ_SERVER_URL"),
                profile,
            )
            .unwrap()
        );
        let account_runtime = std::env::var(ENV_ACCOUNT_URL).ok();
        let account = configured_account().unwrap();
        assert_eq!(
            account,
            resolve_account_url(
                account_runtime.as_deref(),
                option_env!("BUWIZ_ACCOUNT_URL"),
                profile,
            )
            .unwrap()
        );
        if nonempty(server_runtime.as_deref()).is_none()
            && nonempty(option_env!("BUWIZ_SERVER_URL")).is_none()
        {
            assert_eq!(server.url, profile_server_url(profile));
            assert_eq!(server.source, UrlSource::Default);
        }
        if nonempty(account_runtime.as_deref()).is_none()
            && nonempty(option_env!("BUWIZ_ACCOUNT_URL")).is_none()
        {
            assert_eq!(account.url, profile_account_url(profile));
            assert_eq!(account.source, UrlSource::Default);
        }
    }

    #[test]
    fn device_flow_waits_then_returns_tokens_without_logging_them() {
        let mut http = Scripted {
            steps: vec![
                Ok(HttpResponse {
                    status: 200,
                    body: r#"{"access_token":"access-secret","refresh_token":"refresh-secret","expires_in":900}"#
                        .into(),
                }),
                Ok(HttpResponse {
                    status: 400,
                    body: r#"{"error":"slow_down","error_description":"slow down"}"#.into(),
                }),
                Ok(HttpResponse {
                    status: 400,
                    body: r#"{"error":"authorization_pending"}"#.into(),
                }),
            ],
            seen: Vec::new(),
        };
        let started = start_device(
            &mut Scripted {
                steps: vec![Ok(HttpResponse {
                    status: 200,
                    body: r#"{"device_code":"device-1","user_code":"ABCD-EFGH","verification_uri":"http://localhost:3008/login/device","verification_uri_complete":"http://localhost:3008/login/device?user_code=ABCD-EFGH","expires_in":900,"interval":5}"#.into(),
                })],
                seen: Vec::new(),
            },
            "http://localhost:3008",
            1_000,
        )
        .unwrap();
        assert_eq!(started.user_code, "ABCD-EFGH");
        assert_eq!(started.interval, Duration::from_secs(5));

        let now = Cell::new(1_000i64);
        let mut slept = Vec::new();
        let tokens = poll_until(&mut http, &started, &mut || now.get(), &mut |delay| {
            slept.push(delay);
            now.set(now.get() + delay.as_secs() as i64);
        })
        .unwrap();
        assert_eq!(tokens.access_token, "access-secret");
        assert_eq!(slept, vec![Duration::from_secs(5), Duration::from_secs(10)]);
        assert!(
            http.seen
                .iter()
                .all(|line| line.ends_with("/auth/device/poll"))
        );
        let secret_debug = format!(
            "{:?}",
            AccountSecrets {
                access_token: tokens.access_token,
                refresh_token: tokens.refresh_token,
                ..secrets()
            }
        );
        assert!(!secret_debug.contains("access-secret"));
        assert!(!secret_debug.contains("refresh-secret"));
    }

    #[test]
    fn pending_until_the_deadline_times_out() {
        struct AlwaysPending;
        impl Http for AlwaysPending {
            fn exchange(&mut self, _: &HttpRequest) -> Result<HttpResponse, CloudError> {
                Ok(HttpResponse {
                    status: 400,
                    body: r#"{"error":"authorization_pending"}"#.into(),
                })
            }
        }
        let now = Cell::new(9_990i64);
        let error = poll_until(
            &mut AlwaysPending,
            &challenge(),
            &mut || now.get(),
            &mut |delay| now.set(now.get() + delay.as_secs() as i64),
        )
        .unwrap_err();
        assert_eq!(error, CloudError::TimedOut);
    }

    #[test]
    fn refresh_replaces_tokens_and_invalid_grant_signs_out() {
        let mut http = Scripted {
            steps: vec![
                Ok(HttpResponse {
                    status: 200,
                    body: r#"{"user_id":"user-1","email":"filer@example.test","orgs":[]}"#.into(),
                }),
                Ok(HttpResponse {
                    status: 200,
                    body: r#"{"access_token":"access-next","refresh_token":"refresh-next","expires_in":900}"#
                        .into(),
                }),
            ],
            seen: Vec::new(),
        };
        let mut current = secrets();
        current.access_expires_at_unix = 1;
        let SessionUpdate::Online(next) = refresh_session(&mut http, &current, 1_000).unwrap()
        else {
            panic!("expected a fresh session");
        };
        assert_eq!(next.access_token, "access-next");
        assert_eq!(next.refresh_token, "refresh-next");
        assert!(!next.offline);
        assert_eq!(http.seen[0], "POST http://localhost:3008/oauth/token");
        assert_eq!(http.seen[1], "GET http://localhost:3008/me");

        let mut denied = Scripted {
            steps: vec![Ok(HttpResponse {
                status: 400,
                body: r#"{"error":"invalid_grant","error_description":"refresh token revoked"}"#
                    .into(),
            })],
            seen: Vec::new(),
        };
        assert!(matches!(
            refresh_session(&mut denied, &current, 1_000).unwrap(),
            SessionUpdate::SignedOut
        ));
    }

    #[test]
    fn buwiz_servers_own_token_errors_are_a_rejected_sign_in() {
        // buwiz-server's error body, at any 4xx or 5xx status, and a
        // plain-text rejection: each is "sign in again", never a retry.
        for (status, body) in [
            (400, r#"{"error":{"code":"invalid_token","message":"access token is invalid"}}"#),
            (500, r#"{"error":{"code":"invalid_grant","message":"refresh token revoked"}}"#),
            (502, "token is invalid"),
        ] {
            assert_eq!(
                interpret_token_response(status, body, false).err(),
                Some(CloudError::InvalidGrant),
                "{status} {body}"
            );
        }
        assert!(matches!(
            interpret_token_response(503, "upstream unavailable", false),
            Err(CloudError::Server { .. })
        ));
    }

    #[test]
    fn the_refresh_lock_is_exclusive_and_the_ended_sign_in_is_remembered() {
        // The account file is a scratch path; the keychain is never touched.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(ACCOUNT_FILE_NAME);
        temp_env::with_var(ENV_ACCOUNT_FILE, Some(path.to_str().unwrap()), || {
            let held = lock_refresh().unwrap();
            let lock = std::fs::OpenOptions::new()
                .write(true)
                .open(path.with_extension("lock"))
                .unwrap();
            assert!(matches!(lock.try_lock(), Err(std::fs::TryLockError::WouldBlock)));
            drop(held);
            assert!(lock.try_lock().is_ok());
            lock.unlock().unwrap();

            record_sign_in_ended("Buwiz rejected the saved sign-in.");
            assert_eq!(
                sign_in_ended().as_deref(),
                Some("Buwiz rejected the saved sign-in.")
            );
            assert!(path.with_extension("ended").exists());
        });
    }

    #[test]
    fn network_failure_keeps_the_session_offline() {
        let mut http = Scripted {
            steps: vec![Err(CloudError::Network("connection refused".into()))],
            seen: Vec::new(),
        };
        let current = secrets();
        let SessionUpdate::Offline(next) = refresh_session(&mut http, &current, 100).unwrap()
        else {
            panic!("expected offline");
        };
        assert!(next.offline);
        assert_eq!(next.refresh_token, current.refresh_token);
        assert_eq!(next.email.as_deref(), Some("filer@example.test"));
    }

    #[test]
    fn status_hides_tokens_and_a_session_for_a_different_server() {
        let resolved =
            resolve_server_url(Some("http://localhost:3008"), None, BuildProfile::Release).unwrap();
        let status = status_from(&resolved, Some(&secrets()));
        let json = serde_json::to_string(&status).unwrap();
        assert!(status.signed_in);
        assert_eq!(status.email.as_deref(), Some("filer@example.test"));
        assert!(!json.contains("access-secret"));
        assert!(!json.contains("refresh-secret"));
        assert!(!json.contains("access_token"));

        let other =
            resolve_server_url(Some("https://api.buwiz.com"), None, BuildProfile::Debug).unwrap();
        let hidden = status_from(&other, Some(&secrets()));
        assert!(!hidden.signed_in);
        assert!(hidden.email.is_none());
    }

    #[test]
    fn request_body_header_uses_standard_base64() {
        assert_eq!(standard_b64(b"hi"), "aGk=");
        assert_eq!(standard_b64(b"{\"a\":1}"), "eyJhIjoxfQ==");
        assert_eq!(standard_b64(b""), "");
    }

    #[test]
    fn stored_account_round_trips_and_rejects_a_partial_record() {
        let stored = StoredAccount::from_secrets(&secrets());
        let encoded = stored.encode().unwrap();
        assert!(!encoded.contains("debug"));
        let decoded = StoredAccount::decode(&encoded).unwrap();
        assert_eq!(decoded.refresh_token, "refresh-secret");
        assert_eq!(decoded.last_refresh_unix, 0);
        assert!(StoredAccount::decode(r#"{"server_url":"http://localhost:3008"}"#).is_err());
        let legacy = StoredAccount::decode(
            r#"{"server_url":"http://localhost:3008","access_token":"a","refresh_token":"r","access_expires_at_unix":1,"user_id":"u","email":null,"offline":false}"#,
        )
        .unwrap();
        assert_eq!(legacy.last_refresh_unix, 0);
    }

    #[test]
    fn unreachable_server_stays_unlocked_and_a_revoked_token_closes() {
        assert_eq!(
            gate_after_refresh(RefreshOutcome::Online),
            GateDecision::Open { offline: false }
        );
        assert_eq!(
            gate_after_refresh(RefreshOutcome::Offline),
            GateDecision::Open { offline: true }
        );
        assert_eq!(
            gate_after_refresh(RefreshOutcome::Revoked),
            GateDecision::Closed {
                clear_secrets: true
            }
        );
    }

    #[test]
    fn create_account_opens_signup_with_the_device_page_as_next() {
        let sign_in = account_browser_url(
            "http://localhost:3008/",
            AccountBrowser::SignIn,
            "ABCD-EFGH",
        );
        let create = account_browser_url(
            "http://localhost:3008",
            AccountBrowser::CreateAccount,
            "ABCD-EFGH",
        );
        assert_eq!(
            sign_in,
            "http://localhost:3008/login?next=%2Fdevice%3Fuser_code%3DABCD-EFGH"
        );
        assert_eq!(
            create,
            "http://localhost:3008/signup?next=%2Fdevice%3Fuser_code%3DABCD-EFGH"
        );
    }

    #[test]
    fn the_private_file_round_trips_when_the_keychain_has_no_account() {
        let previous = keyring_load();
        keyring_delete();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(ACCOUNT_FILE_NAME);
        temp_env::with_var(ENV_ACCOUNT_FILE, Some(path.to_str().unwrap()), || {
            let stored = StoredAccount::from_secrets(&secrets());
            save_stored_account(&stored).unwrap();
            let loaded = load_stored_account().unwrap().expect("saved account");
            assert_eq!(loaded.refresh_token, "refresh-secret");
            assert_eq!(loaded.email.as_deref(), Some("filer@example.test"));
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o600);
            }
            clear_stored_account();
            assert!(load_stored_account().unwrap().is_none());
            assert!(!path.exists());
        });
        if let Some(raw) = previous {
            let _ = keyring_save(&raw);
        }
    }
}
