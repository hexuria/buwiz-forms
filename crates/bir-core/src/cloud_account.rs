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
//! On Android and on iPhone and iPad there is no keychain entry: the private
//! file alone holds the sign-in, in the app's own folder (on iOS excluded
//! from backups and protected "complete until first user authentication").

use std::io::Write;
use std::path::PathBuf;
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
    /// Plain `http://` to a host that is not this computer: the sign-in code and the tokens would cross the
    /// network unencrypted.
    PlainHttp,
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
            Self::PlainHttp => write!(
                f,
                "The server address must start with https://. Plain http:// is only for a server on this computer."
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloudError {
    Network(String),
    InvalidGrant,
    TimedOut,
    /// The person stopped the wait (Cancel). Not a failure: nothing is shown for it.
    Cancelled,
    Server {
        description: String,
    },
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
            Self::TimedOut => write!(
                f,
                "The sign-in code expired before it was approved. Press Sign in again for a new code."
            ),
            Self::Cancelled => write!(f, "Sign-in was cancelled."),
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
    /// The app-store reviewers' login: every send is a dry run.
    pub practice: bool,
}

impl std::fmt::Debug for AccountSecrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountSecrets")
            .field("server_url", &self.server_url)
            .field("user_id", &self.user_id)
            .field("email", &self.email)
            .field("offline", &self.offline)
            .field("practice", &self.practice)
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
    pub practice: bool,
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

#[derive(Clone)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub content_type: &'static str,
    pub bearer: Option<String>,
    pub body: String,
}

/// Never prints the bearer token or the body (a form body can carry a refresh token or a code).
impl std::fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("content_type", &self.content_type)
            .field("bearer", &self.bearer.as_ref().map(|_| "<redacted>"))
            .field("body_bytes", &self.body.len())
            .finish()
    }
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
    /// Kept so an offline launch stays a dry run. A session saved before this
    /// field existed loads as `false` until the next online refresh.
    #[serde(default)]
    pub practice: bool,
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
            practice: self.practice,
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
            practice: secrets.practice,
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

/// API origin. `runtime` is the process's `BUWIZ_SERVER_URL` as [`crate::config::var`] reads it (the
/// environment, or an Android development build's baked list).
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
    let runtime = crate::config::var(ENV_SERVER_URL).ok();
    resolve_server_url(
        runtime.as_deref(),
        option_env!("BUWIZ_SERVER_URL"),
        active_build_profile(),
    )
}

/// Where the browser opens login and signup.
pub fn configured_account() -> Result<ResolvedServer, ServerUrlError> {
    let runtime = crate::config::var(ENV_ACCOUNT_URL).ok();
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
    // The sign-in code and the tokens travel to this address. Unencrypted, only to this computer itself (a local
    // development or mock server); to anywhere else it must be https, in every build.
    if url.scheme() == "http" && !this_computer(host) {
        return Err(ServerUrlError::PlainHttp);
    }
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

/// `host` (no brackets) is this computer: `localhost` or a loopback address.
fn this_computer(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
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
            practice: false,
            detail: "Not signed in.".into(),
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
        practice: secrets.practice,
        detail,
    }
}

/// Starts the browser's device flow. `install_id` ([`crate::install_id`]) is sent from this first request.
pub fn start_device(
    http: &mut dyn Http,
    server_url: &str,
    now_unix: i64,
    install_id: &str,
) -> Result<DeviceChallenge, CloudError> {
    let response = http.exchange(&HttpRequest {
        method: "POST",
        url: format!("{server_url}/auth/device/start"),
        content_type: "application/json",
        bearer: None,
        body: json!({ "client_id": DESKTOP_CLIENT_ID, "install_id": install_id }).to_string(),
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
    poll_until_stopped(http, challenge, now, sleep, &mut || false)
}

/// [`poll_until`] that the person can stop: `stopped` is asked before every
/// try, and a stopped wait ends as [`CloudError::Cancelled`]. A caller that
/// wants a prompt stop sleeps in short slices ([`sleep_unless_stopped`]).
pub fn poll_until_stopped(
    http: &mut dyn Http,
    challenge: &DeviceChallenge,
    now: &mut dyn FnMut() -> i64,
    sleep: &mut dyn FnMut(Duration),
    stopped: &mut dyn FnMut() -> bool,
) -> Result<TokenSet, CloudError> {
    // 900s / 1s, with room for slow_down. A frozen clock still stops.
    for _ in 0..1000 {
        if stopped() {
            return Err(CloudError::Cancelled);
        }
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

/// Sleeps `total` in pieces of at most `slice`, and stops early once
/// `stopped` says so: a Cancel is answered within one slice.
pub fn sleep_unless_stopped(
    total: Duration,
    slice: Duration,
    stopped: &mut dyn FnMut() -> bool,
    sleep: &mut dyn FnMut(Duration),
) {
    let mut left = total;
    while !left.is_zero() && !stopped() {
        let piece = left.min(slice);
        sleep(piece);
        left = left.saturating_sub(piece);
    }
}

/// The approval page's address as a person reads and types it: no scheme,
/// no trailing slash (`account.buwiz.com/device`, `localhost:3008/device`).
pub fn short_address(uri: &str) -> String {
    let trimmed = uri.trim();
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    rest.trim_end_matches('/').to_owned()
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

/// `GET /me`: user id, email, and whether this is a practice account. An
/// older server omits `practice`; anything but `true` means a real account.
pub fn load_profile(
    http: &mut dyn Http,
    server_url: &str,
    access_token: &str,
) -> Result<(String, Option<String>, bool), CloudError> {
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
    let practice = value
        .get("practice")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok((user_id, email, practice))
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
            Ok((user_id, email, practice)) => {
                let mut next = secrets.clone();
                next.user_id = user_id;
                next.email = email;
                next.offline = false;
                next.practice = practice;
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
            let (user_id, email, practice) =
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
                practice,
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
    let _ = send_logout(http, &secrets.server_url, &secrets.access_token);
}

/// `POST /api/auth/logout` with `access_token`: the HTTP status the server answered. It needs nothing else of
/// the sign-in, so a caller that sends it later keeps only the address and that one token.
pub fn send_logout(
    http: &mut dyn Http,
    server_url: &str,
    access_token: &str,
) -> Result<u16, CloudError> {
    http.exchange(&HttpRequest {
        method: "POST",
        url: format!("{server_url}/api/auth/logout"),
        content_type: "application/json",
        bearer: Some(access_token.to_owned()),
        body: "{}".into(),
    })
    .map(|response| response.status)
}

/// Opens the account site in the system browser. On a phone the app's
/// registered opener does it ([`crate::platform::set_url_opener`]), and only
/// for an address [`crate::platform::browser_url_allowed`] accepts: the
/// `open` crate starts a helper program there (`xdg-open` on Android,
/// `uiopen` on iOS), which a phone app has none of and may not start.
pub fn open_browser(url: &str) -> Result<(), String> {
    if let Some(open) = crate::platform::url_opener() {
        if !crate::platform::browser_url_allowed(url) {
            return Err(format!("This address is not opened in the browser: {url}"));
        }
        return open(url);
    }
    if cfg!(any(target_os = "ios", target_os = "android")) {
        return Err("This app cannot open the browser here.".into());
    }
    open::that(url).map_err(|error| error.to_string())
}

/// Test instrumentation: how many times the stored account was read from the disk or keychain,
/// so the owner cache's tests can prove a read never happened.
#[cfg(any(test, feature = "test-support"))]
static STORED_ACCOUNT_LOADS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many stored-account loads this process ran.
#[cfg(any(test, feature = "test-support"))]
pub fn stored_account_loads() -> u64 {
    STORED_ACCOUNT_LOADS.load(std::sync::atomic::Ordering::SeqCst)
}

/// Load the desktop account. The keychain wins when it holds a complete
/// record. Otherwise the private file is used. `Ok(None)` means signed out.
pub fn load_stored_account() -> Result<Option<StoredAccount>, CloudError> {
    #[cfg(any(test, feature = "test-support"))]
    STORED_ACCOUNT_LOADS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if let Some(raw) = keyring_load() {
        if raw.trim() == SIGNED_OUT_SENTINEL.trim() {
            return Ok(None);
        }
        if let Ok(stored) = StoredAccount::decode(&raw) {
            return Ok(Some(stored));
        }
    }
    let path = account_file_path();
    // #4: try_exists, not exists() — a stat ERROR (EACCES, EIO) is a read error, never
    // "nobody"; exists() would read it as nobody and the daemon would exit for an error.
    let exists = path.try_exists().map_err(|error| {
        CloudError::Unexpected(format!("could not read the saved Buwiz account ({error})"))
    })?;
    if !exists {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|error| {
        CloudError::Unexpected(format!("could not read the saved Buwiz account ({error})"))
    })?;
    // A definite nobody is the file ABSENT (handled above) or the explicit sign-out
    // sentinel. Anything else unreadable — zero bytes included — is a READ ERROR, never
    // nobody: a torn read of the in-place fallback is exactly 0 bytes.
    if raw.trim() == SIGNED_OUT_SENTINEL.trim() {
        return Ok(None);
    }
    StoredAccount::decode(&raw).map(Some)
}

/// The stored sign-in for a process that re-reads it per unit of work (the headless daemon's
/// per-batch check, #4): the read holds the account-session lock the save writes under, so a
/// same-owner refresh is never read back in its write window; and a file whose modification
/// time has not changed answers from memory — no keychain call per batch.
///
/// The cache key is the file's modification time. A sign-in that lives only in the keychain
/// (no file) is never cache-shaped, so those reads always go through.
pub fn load_stored_account_shared() -> Result<Option<StoredAccount>, CloudError> {
    use std::sync::Mutex;
    type Stamp = (Option<std::time::SystemTime>, u64, String);
    static CACHE: Mutex<Option<(std::path::PathBuf, Stamp, Option<StoredAccount>)>> =
        Mutex::new(None);
    let _account = crate::account_session::lock();
    let path = account_file_path();
    // N6: not the modification time alone — the length too, and the inode where the platform
    // has one: a rewrite that lands inside one mtime tick still changes one of them.
    let stamp = std::fs::metadata(&path).ok().map(|meta| {
        let mtime = meta.modified().ok();
        #[cfg(unix)]
        let extra = {
            use std::os::unix::fs::MetadataExt;
            // the inode alone does not turn over on a same-length rewrite in place; the ctime
            // does — every write moves it — so inode + ctime is the cache key here (Windows uses mtime + length + created)
            format!("{}:{}:{}", meta.ino(), meta.ctime(), meta.ctime_nsec())
        };
        #[cfg(not(unix))]
        let extra = meta
            .created()
            .ok()
            .map(|at| format!("{at:?}"))
            .unwrap_or_default();
        (mtime, meta.len(), extra)
    });
    // A file exists but its mtime cannot be read: never cache-shaped. No file at all: the
    // keychain alone decides, again never cache-shaped — either way read through.
    if path.exists() && stamp.as_ref().and_then(|(mtime, _, _)| *mtime).is_some() {
        let stamp = stamp.expect("checked above");
        let mut cache = CACHE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((seen_path, seen_stamp, cached)) = cache.as_ref()
            && *seen_path == path
            && *seen_stamp == stamp
        {
            return Ok(cached.clone());
        }
        let fresh = load_stored_account()?;
        *cache = Some((path.clone(), stamp, fresh.clone()));
        return Ok(fresh);
    }
    let fresh = load_stored_account()?;
    // The file having gone away invalidates any cached answer keyed by its old mtime only when
    // the file comes back with the same stamp — a vanishingly wide window nobody can hit on
    // purpose, and the next real write moves the stamp. Still, drop it: cheap and exact.
    if let Ok(mut cache) = CACHE.lock() {
        *cache = None;
    }
    Ok(fresh)
}

/// Write the desktop account to the keychain and the private file.
/// The file is always written: a keyring build without a real backend can
/// report success and then forget the secret when the process exits.
pub(crate) fn save_stored_account(stored: &StoredAccount) -> Result<(), CloudError> {
    let raw = stored.encode()?;
    let _ = keyring_save(&raw);
    write_secret_file(&raw)
}

/// Remove the keychain entry and the private file.
pub(crate) fn clear_stored_account() {
    keyring_delete();
    // N10: the atomic save's own temporary files hold the same refresh token while they
    // lie around — a write cut off mid-save leaves one — so sign-out removes them too.
    sweep_write_temps_scoped(&account_file_path(), true);
    #[cfg(test)]
    if FAIL_ACCOUNT_REMOVE_FOR_TEST.load(std::sync::atomic::Ordering::SeqCst) {
        // the simulated failure: the remove does not happen, as the OS's would not
        return;
    }
    let _ = std::fs::remove_file(account_file_path());
}

/// The save's sibling temporary files (`<file>.write-*`), swept at the start of every save
/// (a crashed earlier write leaves one behind) and on sign-out (N10).
fn sweep_write_temps(path: &std::path::Path) {
    sweep_write_temps_scoped(path, false);
}

/// #6: the lock is in-process only, so an unscoped sweep would delete ANOTHER process's
/// in-flight temp (the app's, while the daemon saves) — on Unix the app's rename then gets
/// ENOENT and the save fails with the old token kept; on a desktop without a keyring the
/// rotated refresh token is lost. A save sweeps only this process's own temps, temps whose
/// pid is dead, and temps an hour old; sign-out sweeps all, as nothing of the account may
/// stay.
fn sweep_write_temps_scoped(path: &std::path::Path, all: bool) {
    let Some(folder) = path.parent() else {
        return;
    };
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return;
    };
    let prefix = format!("{name}.write-");
    let own = std::process::id().to_string();
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let Some(file) = entry.file_name().to_str().map(|f| f.to_owned()) else {
                continue;
            };
            let Some(pid) = file.strip_prefix(&prefix) else {
                continue;
            };
            let take = all
                || pid == own
                || pid.parse::<u32>().is_ok_and(|pid| !pid_alive(pid))
                || entry
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|at| at.elapsed().ok())
                    .is_some_and(|age| age.as_secs() > 3600);
            if take {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    // kill(pid, 0): the process exists — EPERM means alive but not ours
    unsafe {
        libc::kill(pid as i32, 0) == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    // no portable liveness probe here: a foreign temp is kept unless it is old — safe side
    true
}

/// Remove the saved sign-in for a sign-out, and make sure it is gone: read back, removed once more, then
/// overwritten with nothing where it still reads (an empty keychain entry or file is not a sign-in). An error
/// means a sign-in still reads back on this device.
/// The explicit "signed out" record a sign-out writes when the file cannot be removed (the
/// ruling): a definite nobody that decodes as nobody, never as a torn read. Zero bytes is NOT
/// a nobody — the in-place truncate-then-write fallback leaves exactly that mid-write on a
/// torn read, and reading it as signed-out would exit the daemon on a same-owner refresh and
/// sign the app's own user out for nothing (on Windows the fallback is common: a rename onto
/// a file a scanner or the indexer holds open is ACCESS_DENIED).
pub(crate) const SIGNED_OUT_SENTINEL: &str = "{\"signed_out\":true}\n";

#[cfg(test)]
pub(crate) static FAIL_ACCOUNT_REMOVE_FOR_TEST: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(crate) fn forget_stored_account() -> Result<(), CloudError> {
    #[cfg(test)]
    let remove_fails = FAIL_ACCOUNT_REMOVE_FOR_TEST.load(std::sync::atomic::Ordering::SeqCst);
    #[cfg(not(test))]
    let remove_fails = false;
    let _ = remove_fails;
    clear_stored_account();
    if !sign_in_reads_back() {
        return Ok(());
    }
    clear_stored_account();
    if !sign_in_reads_back() {
        return Ok(());
    }
    if keyring_load().is_some() {
        let _ = keyring_save(SIGNED_OUT_SENTINEL);
    }
    if remove_fails || account_file_path().exists() {
        let _ = write_secret_file(SIGNED_OUT_SENTINEL);
    }
    if sign_in_reads_back() {
        return Err(CloudError::Unexpected(
            "the saved Buwiz sign-in could not be removed".into(),
        ));
    }
    Ok(())
}

fn sign_in_reads_back() -> bool {
    matches!(load_stored_account(), Ok(Some(_)))
}

/// The saved sign-in's file: in the device folder (`platform::device_dir`), whichever room is open.
pub(crate) fn account_file_path() -> PathBuf {
    if let Some(path) = std::env::var_os(ENV_ACCOUNT_FILE) {
        return PathBuf::from(path);
    }
    crate::platform::device_dir().join(ACCOUNT_FILE_NAME)
}

#[cfg(test)]
static FAIL_ATOMIC_FOR_TEST: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn write_secret_file(raw: &str) -> Result<(), CloudError> {
    let path = account_file_path();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            CloudError::Unexpected(format!("could not store the Buwiz account ({error})"))
        })?;
    }
    // Atomic (#4): the content lands in a sibling temporary file first and one rename puts it
    // over the live one — a reader (the headless daemon's per-batch check) sees the old sign-in
    // whole or the new one whole, never the empty or torn window of a truncate-then-write.
    sweep_write_temps(&path);
    let mut temp_name = path.as_os_str().to_os_string();
    temp_name.push(format!(".write-{}", std::process::id()));
    let temp = std::path::PathBuf::from(temp_name);
    let mut permission_denied = false;
    let written = (|| -> Result<(), CloudError> {
        let mut deny = |error: &std::io::Error| {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                permission_denied = true;
            }
        };
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp)
            .map_err(|error| {
                deny(&error);
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
        let _ = file.sync_all();
        drop(file);
        // std::fs::rename replaces an existing target on Windows too (MoveFileExW with
        // REPLACE_EXISTING in std since 1.55): no remove-then-rename, so no window where the
        // file is simply gone (N4 — a daemon reading in that window saw nobody signed in and
        // exited on a same-owner refresh).
        #[cfg(test)]
        if FAIL_ATOMIC_FOR_TEST.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = std::fs::remove_file(&temp);
            // a full disk: neither a permission refusal nor a read-only folder
            return Err(CloudError::Unexpected(
                "could not store the Buwiz account (No space left on device (os error 28))".into(),
            ));
        }
        std::fs::rename(&temp, &path).map_err(|error| {
            deny(&error);
            CloudError::Unexpected(format!("could not store the Buwiz account ({error})"))
        })?;
        #[cfg(unix)]
        if let Some(folder) = path.parent().filter(|p| !p.as_os_str().is_empty())
            && let Ok(folder) = std::fs::File::open(folder)
        {
            let _ = folder.sync_all();
        }
        Ok(())
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    if let Err(atomic_error) = written {
        // N5: the in-place fallback is for the cases that mean "the folder, not the disk":
        // the sign-out's own overwrite — the sentinel today, an empty write of before — (it
        // must land even in a read-only folder, and it destroys nothing a truncation could
        // lose: S1 — without the sentinel here, a remove that failed AND an atomic write
        // failing for another reason, a full disk or a sharing violation, would leave the
        // OLD ACCOUNT FILE in place and the user signed in after signing out) and a
        // permission refusal (a read-only folder: the temp or the rename is refused though
        // the file itself is writable). ANY other error — a full disk above all — leaves
        // the live file UNTOUCHED: truncating it first and then failing would destroy the
        // sign-in the atomic write exists to protect.
        if raw.is_empty() || raw.trim() == SIGNED_OUT_SENTINEL.trim() || permission_denied {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&path)
                .map_err(|error| {
                    CloudError::Unexpected(format!(
                        "could not store the Buwiz account ({atomic_error}; {error})"
                    ))
                })?;
            file.write_all(raw.as_bytes()).map_err(|error| {
                CloudError::Unexpected(format!("could not store the Buwiz account ({error})"))
            })?;
            let _ = file.sync_all();
        } else {
            return Err(atomic_error);
        }
    }
    bir_submit::ios_storage::protect_file(&path);
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn keyring_entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
}

// `BUWIZ_NO_KEYCHAIN=1`: no call below reaches the OS keychain. A read finds
// nothing (the private file, if any, still answers), a write reports failure
// (the private file is still written), a delete has nothing to delete.

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn keyring_load() -> Option<String> {
    if crate::platform::keychain_disabled() {
        return None;
    }
    let entry = keyring_entry().ok()?;
    match entry.get_password() {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn keyring_save(raw: &str) -> bool {
    if crate::platform::keychain_disabled() {
        return false;
    }
    keyring_entry()
        .and_then(|entry| entry.set_password(raw))
        .is_ok()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn keyring_delete() {
    if crate::platform::keychain_disabled() {
        return;
    }
    if let Ok(entry) = keyring_entry() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => tracing::debug!("Buwiz keychain entry not deleted: {error}"),
        }
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn keyring_load() -> Option<String> {
    None
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn keyring_save(_raw: &str) -> bool {
    false
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn keyring_delete() {}

pub struct BlockingHttp {
    client: reqwest::blocking::Client,
}

impl BlockingHttp {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self { client }
    }
}

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
    let error = value.get("error").and_then(Value::as_str).unwrap_or("");
    if allow_pending && error == "authorization_pending" {
        return Ok(Poll::Pending);
    }
    if allow_pending && error == "slow_down" {
        return Ok(Poll::SlowDown);
    }
    if error == "invalid_grant" || error == "invalid_token" || status == 401 {
        return Err(CloudError::InvalidGrant);
    }
    if allow_pending && error == "expired_token" {
        // The device code ran out on the server's own clock.
        return Err(CloudError::TimedOut);
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
    #[test]
    fn a_request_never_debug_prints_its_bearer_or_body() {
        let request = HttpRequest {
            method: "POST",
            url: "https://example.com/token".into(),
            content_type: "application/x-www-form-urlencoded",
            bearer: Some("access-secret-fixture".into()),
            body: "refresh_token=refresh-secret-fixture".into(),
        };
        let printed = format!("{request:?}");
        assert!(!printed.contains("access-secret-fixture"), "{printed}");
        assert!(!printed.contains("refresh-secret-fixture"), "{printed}");
        assert!(printed.contains("https://example.com/token"), "{printed}");
        assert!(printed.contains("<redacted>"), "{printed}");
    }

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
            practice: false,
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
        // Plain http is for this computer only: the sign-in code and the tokens go to this address.
        for local in [
            "http://localhost:3008",
            "http://127.0.0.1:3008",
            "http://LOCALHOST",
        ] {
            assert!(normalize_server_url(local).is_ok(), "{local}");
        }
        for elsewhere in [
            "http://api.buwiz.com",
            "http://192.0.2.1:3008",
            "http://10.0.2.2:3008",
            "http://localhost.example",
            "http://[2001:db8::1]:3008",
        ] {
            assert_eq!(
                normalize_server_url(elsewhere),
                Err(ServerUrlError::PlainHttp),
                "{elsewhere}"
            );
        }
        assert_eq!(
            normalize_server_url("https://192.0.2.1:3008").unwrap(),
            "https://192.0.2.1:3008"
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
        let server_runtime = crate::config::var(ENV_SERVER_URL).ok();
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
        let account_runtime = crate::config::var(ENV_ACCOUNT_URL).ok();
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
            "install-id-of-this-test-0001",
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

    /// A phone freezes the app while the person is in the browser: the wall clock jumps, and the first poll after
    /// the app comes back can fail on a dead connection. That poll is tried again, and the sign-in finishes.
    #[test]
    fn a_network_error_after_a_frozen_gap_is_tried_again() {
        let mut http = Scripted {
            // Popped from the end: pending, then a dead connection after the gap, then approved.
            steps: vec![
                Ok(HttpResponse {
                    status: 200,
                    body: r#"{"access_token":"access-secret","refresh_token":"refresh-secret","expires_in":900}"#
                        .into(),
                }),
                Err(CloudError::Network("connection reset by peer".into())),
                Ok(HttpResponse {
                    status: 400,
                    body: r#"{"error":"authorization_pending"}"#.into(),
                }),
            ],
            seen: Vec::new(),
        };
        let now = Cell::new(9_000i64);
        let mut sleeps = 0;
        let tokens = poll_until(&mut http, &challenge(), &mut || now.get(), &mut |delay| {
            sleeps += 1;
            // The first wait is the frozen one: four minutes pass on the wall clock.
            let gap = if sleeps == 1 {
                240
            } else {
                delay.as_secs() as i64
            };
            now.set(now.get() + gap);
        })
        .unwrap();
        assert_eq!(tokens.access_token, "access-secret");
        assert_eq!(http.seen.len(), 3);
    }

    /// A gap that outlasts the code's expiry (by the wall clock) ends with the expired message, without another poll.
    #[test]
    fn a_frozen_gap_past_the_codes_expiry_ends_as_expired() {
        let mut http = Scripted {
            steps: vec![Ok(HttpResponse {
                status: 400,
                body: r#"{"error":"authorization_pending"}"#.into(),
            })],
            seen: Vec::new(),
        };
        let now = Cell::new(9_000i64);
        let error = poll_until(&mut http, &challenge(), &mut || now.get(), &mut |_| {
            now.set(now.get() + 2_000)
        })
        .unwrap_err();
        assert_eq!(error, CloudError::TimedOut);
        assert_eq!(http.seen.len(), 1);
        assert!(error.to_string().contains("code expired"));
    }

    /// A clock that never moves and a server that is never reached still end: the wait has a bound of its own.
    #[test]
    fn endless_network_errors_with_a_stuck_clock_still_end() {
        struct Unreachable(usize);
        impl Http for Unreachable {
            fn exchange(&mut self, _: &HttpRequest) -> Result<HttpResponse, CloudError> {
                self.0 += 1;
                Err(CloudError::Network("no route to host".into()))
            }
        }
        let mut http = Unreachable(0);
        let error = poll_until(&mut http, &challenge(), &mut || 1_000, &mut |_| {}).unwrap_err();
        assert_eq!(error, CloudError::TimedOut);
        assert_eq!(http.0, 1000);
    }

    #[test]
    fn a_stopped_wait_ends_as_cancelled_before_the_next_try() {
        struct AlwaysPending(usize);
        impl Http for AlwaysPending {
            fn exchange(&mut self, _: &HttpRequest) -> Result<HttpResponse, CloudError> {
                self.0 += 1;
                Ok(HttpResponse {
                    status: 400,
                    body: r#"{"error":"authorization_pending"}"#.into(),
                })
            }
        }
        let mut http = AlwaysPending(0);
        let mut tries_left = 2;
        let error = poll_until_stopped(
            &mut http,
            &challenge(),
            &mut || 1_000,
            &mut |_| {},
            &mut || {
                if tries_left == 0 {
                    return true;
                }
                tries_left -= 1;
                false
            },
        )
        .unwrap_err();
        assert_eq!(error, CloudError::Cancelled);
        assert_eq!(http.0, 2);
    }

    #[test]
    fn a_sliced_sleep_stops_within_one_slice() {
        let mut slept = Vec::new();
        let mut asked = 0;
        sleep_unless_stopped(
            Duration::from_secs(5),
            Duration::from_millis(200),
            &mut || {
                asked += 1;
                asked > 3
            },
            &mut |piece| slept.push(piece),
        );
        assert_eq!(slept, vec![Duration::from_millis(200); 3]);
        let mut whole = Duration::ZERO;
        sleep_unless_stopped(
            Duration::from_millis(450),
            Duration::from_millis(200),
            &mut || false,
            &mut |piece| whole += piece,
        );
        assert_eq!(whole, Duration::from_millis(450));
    }

    #[test]
    fn the_servers_expired_code_ends_as_expired() {
        let mut http = Scripted {
            steps: vec![Ok(HttpResponse {
                status: 400,
                body: r#"{"error":"expired_token","error_description":"unknown or expired device code"}"#.into(),
            })],
            seen: Vec::new(),
        };
        let error = poll_until(&mut http, &challenge(), &mut || 1_000, &mut |_| {}).unwrap_err();
        assert_eq!(error, CloudError::TimedOut);
    }

    #[test]
    fn the_short_address_drops_the_scheme_and_the_last_slash() {
        assert_eq!(
            short_address("https://account.buwiz.com/device"),
            "account.buwiz.com/device"
        );
        assert_eq!(
            short_address("https://account.buwiz.com/device/"),
            "account.buwiz.com/device"
        );
        assert_eq!(
            short_address("http://localhost:3008/device"),
            "localhost:3008/device"
        );
        assert_eq!(short_address(" http://127.0.0.1:3008/ "), "127.0.0.1:3008");
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
        assert!(
            !legacy.practice,
            "a session saved before the field is not practice"
        );
    }

    fn profile_from(body: &str) -> (String, Option<String>, bool) {
        let mut http = Scripted {
            steps: vec![Ok(HttpResponse {
                status: 200,
                body: body.into(),
            })],
            seen: Vec::new(),
        };
        load_profile(&mut http, "http://localhost:3008", "access-secret").unwrap()
    }

    #[test]
    fn the_me_answer_marks_a_practice_account_and_an_older_server_is_not_one() {
        let (_, _, practice) =
            profile_from(r#"{"user_id":"user-1","email":"review@example.test","practice":true}"#);
        assert!(practice);
        let (_, _, practice) = profile_from(r#"{"user_id":"user-1","email":"filer@example.test"}"#);
        assert!(!practice);
        let (_, _, practice) = profile_from(r#"{"user_id":"user-1","practice":"true"}"#);
        assert!(!practice, "only a JSON boolean counts");

        let mut http = Scripted {
            steps: vec![Ok(HttpResponse {
                status: 200,
                body: r#"{"user_id":"user-1","email":"review@example.test","practice":true}"#
                    .into(),
            })],
            seen: Vec::new(),
        };
        let SessionUpdate::Online(next) = refresh_session(&mut http, &secrets(), 100).unwrap()
        else {
            panic!("expected a fresh session");
        };
        assert!(next.practice);
        let stored =
            StoredAccount::decode(&StoredAccount::from_secrets(&next).encode().unwrap()).unwrap();
        assert!(stored.to_secrets().practice);
        let resolved =
            resolve_server_url(Some("http://localhost:3008"), None, BuildProfile::Release).unwrap();
        assert!(status_from(&resolved, Some(&next)).practice);
        assert!(!status_from(&resolved, None).practice);

        let mut offline = Scripted {
            steps: vec![Err(CloudError::Network("connection refused".into()))],
            seen: Vec::new(),
        };
        let SessionUpdate::Offline(kept) = refresh_session(&mut offline, &next, 100).unwrap()
        else {
            panic!("expected offline");
        };
        assert!(kept.practice, "an offline launch stays a practice account");
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
        // A test build has the keychain off: it holds no account, and nothing here reaches the real one.
        assert_eq!(keyring_load(), None);
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
    }

    #[test]
    fn with_the_keychain_off_the_account_is_signed_out_and_the_os_is_not_called() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(ACCOUNT_FILE_NAME);
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                (ENV_ACCOUNT_FILE, Some(path.to_str().unwrap())),
            ],
            || {
                assert_eq!(keyring_load(), None);
                assert!(!keyring_save("not stored"));
                keyring_delete();
                assert!(load_stored_account().unwrap().is_none(), "signed out");

                // The private file is not the keychain: it still works.
                let stored = StoredAccount::from_secrets(&secrets());
                save_stored_account(&stored).unwrap();
                let loaded = load_stored_account().unwrap().expect("file account");
                assert_eq!(loaded.refresh_token, "refresh-secret");
                clear_stored_account();
                assert!(load_stored_account().unwrap().is_none());
            },
        );
    }

    /// A private file that cannot be deleted (its folder is read-only) is overwritten, so it no longer reads as
    /// a sign-in; one that cannot be overwritten either (read-only too) is reported, never taken as removed.
    #[cfg(unix)]
    #[test]
    fn forgetting_a_sign_in_that_cannot_be_deleted_overwrites_it_or_says_so() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &std::path::Path, mode: u32| {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(ACCOUNT_FILE_NAME);
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                (ENV_ACCOUNT_FILE, Some(path.to_str().unwrap())),
            ],
            || {
                save_stored_account(&StoredAccount::from_secrets(&secrets())).unwrap();
                mode(dir.path(), 0o555);
                let overwritten = forget_stored_account();
                let left = load_stored_account();
                mode(dir.path(), 0o755);
                assert!(
                    !matches!(left, Ok(Some(_))),
                    "the sign-in still reads back after the sign-out"
                );
                assert!(
                    overwritten.is_ok(),
                    "an overwritten file was reported as left"
                );

                save_stored_account(&StoredAccount::from_secrets(&secrets())).unwrap();
                mode(&path, 0o400);
                mode(dir.path(), 0o555);
                let stuck = forget_stored_account();
                let left = load_stored_account();
                mode(dir.path(), 0o755);
                mode(&path, 0o600);
                assert!(
                    matches!(left, Ok(Some(_))),
                    "the test could not make the file stick"
                );
                assert!(
                    stuck.is_err(),
                    "a sign-in that still reads back was reported as removed"
                );
                clear_stored_account();
            },
        );
    }

    /// #4: the save is atomic — a temporary file plus a rename — so a reader during a write sees
    /// the old sign-in whole or the new one whole, never a truncated or torn file. Large tokens
    /// widen the write window so a truncate-then-write save is caught within the iterations.
    /// Positive control: every non-empty read the test makes parses as one whole account.
    #[test]
    fn a_reader_during_a_write_never_sees_a_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.json");
        temp_env::with_var(ENV_ACCOUNT_FILE, Some(path.to_str().unwrap()), || {
            let big_a = "a".repeat(1_000_000);
            let big_b = "b".repeat(1_000_000);
            let first = StoredAccount::from_secrets(&AccountSecrets {
                server_url: "http://localhost:3008".into(),
                access_token: big_a,
                refresh_token: "refresh-a".into(),
                access_expires_at_unix: 4_000_000_000,
                user_id: "user-1".into(),
                email: None,
                offline: false,
                practice: true,
            });
            let mut second = first.clone();
            second.access_token = big_b.clone();
            save_stored_account(&first).unwrap();
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let writer_stop = stop.clone();
            let writer = std::thread::spawn(move || {
                let mut flip = false;
                while !writer_stop.load(std::sync::atomic::Ordering::SeqCst) {
                    let account = if flip { &second } else { &first };
                    save_stored_account(account).unwrap();
                    flip = !flip;
                }
            });
            let mut partial = 0usize;
            let mut reads = 0usize;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while std::time::Instant::now() < deadline {
                reads += 1;
                // N4: discriminates on Windows only; the remove was cfg(windows).
                // A missing file is a failure too, never a skip — with the
                // remove-then-rename save this read found no file at all, and a daemon
                // reading in that window saw nobody signed in and exited on a same-owner
                // refresh.
                let raw = match std::fs::read(&path) {
                    Ok(raw) => raw,
                    Err(_) => {
                        partial += 1;
                        continue;
                    }
                };
                // No erase happens in this test — only saves. An EMPTY file is already the torn
                // state of a truncate-then-write save (the window #4's same-owner refresh hits:
                // the read answers None and the daemon would exit for nothing), a partial file
                // is the other half of it.
                let text = String::from_utf8_lossy(&raw);
                if raw.is_empty() || serde_json::from_str::<serde_json::Value>(&text).is_err() {
                    partial += 1;
                }
            }
            stop.store(true, std::sync::atomic::Ordering::SeqCst);
            writer.join().unwrap();
            assert!(reads > 100, "the reader really read: {reads}");
            assert_eq!(partial, 0, "partial reads seen: positive control failed");
        });
    }

    /// The ruling: zero bytes is a READ ERROR, never a nobody — the in-place
    /// truncate-then-write fallback leaves exactly that on a torn read (common on Windows:
    /// ACCESS_DENIED on the rename), and reading it as signed-out would exit the daemon on a
    /// same-owner refresh. Positive control: the sentinel is nobody, a real account reads.
    #[test]
    fn zero_bytes_is_a_read_error_never_a_nobody() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.json");
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                (ENV_ACCOUNT_FILE, Some(path.to_str().unwrap())),
            ],
            || {
                std::fs::write(&path, "").unwrap();
                assert!(
                    load_stored_account().is_err(),
                    "zero bytes is a read error, never nobody"
                );
                std::fs::write(&path, "not json").unwrap();
                assert!(
                    load_stored_account().is_err(),
                    "the undecodable file is a read error too"
                );
                // positive controls: the sentinel is a definite nobody; a real account reads
                std::fs::write(&path, SIGNED_OUT_SENTINEL).unwrap();
                assert!(
                    matches!(load_stored_account(), Ok(None)),
                    "the sentinel is the definite nobody"
                );
                save_stored_account(&StoredAccount::from_secrets(&secrets())).unwrap();
                assert!(load_stored_account().unwrap().is_some());
            },
        );
    }

    /// The ruling: a sign-out whose remove fails leaves the explicit sentinel — a definite
    /// nobody that decodes as nobody — never zero bytes.
    #[test]
    fn a_sign_out_whose_remove_fails_leaves_the_sentinel() {
        use std::sync::atomic::Ordering;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.json");
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                (ENV_ACCOUNT_FILE, Some(path.to_str().unwrap())),
            ],
            || {
                save_stored_account(&StoredAccount::from_secrets(&secrets())).unwrap();
                FAIL_ACCOUNT_REMOVE_FOR_TEST.store(true, Ordering::SeqCst);
                let forgotten = forget_stored_account();
                FAIL_ACCOUNT_REMOVE_FOR_TEST.store(false, Ordering::SeqCst);
                let _ = forgotten;
                let left = std::fs::read_to_string(&path).unwrap();
                assert_eq!(
                    left, SIGNED_OUT_SENTINEL,
                    "the remove failing leaves the sentinel, never zero bytes"
                );
                assert!(
                    matches!(load_stored_account(), Ok(None)),
                    "the sentinel reads as the definite nobody"
                );
            },
        );
    }

    /// S1: the sign-out's sentinel overwrite falls back in place even when the atomic write
    /// fails for a NON-permission reason (the hook's full disk) — otherwise the old account
    /// file would stay and the user would still be signed in after signing out.
    #[test]
    fn a_sign_out_falls_back_to_the_sentinel_on_any_atomic_failure() {
        use std::sync::atomic::Ordering;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.json");
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                (ENV_ACCOUNT_FILE, Some(path.to_str().unwrap())),
            ],
            || {
                struct TestFlagsReset;
                impl Drop for TestFlagsReset {
                    fn drop(&mut self) {
                        FAIL_ATOMIC_FOR_TEST.store(false, Ordering::SeqCst);
                        FAIL_ACCOUNT_REMOVE_FOR_TEST.store(false, Ordering::SeqCst);
                    }
                }
                let _flags_reset = TestFlagsReset;
                save_stored_account(&StoredAccount::from_secrets(&secrets())).unwrap();
                FAIL_ATOMIC_FOR_TEST.store(true, Ordering::SeqCst);
                FAIL_ACCOUNT_REMOVE_FOR_TEST.store(true, Ordering::SeqCst);
                let _ = forget_stored_account();
                let left = std::fs::read_to_string(&path).unwrap();
                assert_eq!(
                    left, SIGNED_OUT_SENTINEL,
                    "the overwrite lands as the sentinel even when the atomic write fails —                      the old account must not stay"
                );
            },
        );
    }

    /// N5: the atomic write failing with anything but a permission refusal — a FULL DISK,
    /// simulated through the test hook — must leave the live file UNTOUCHED. On the base
    /// behavior the in-place fallback ran on any error: it truncated the live file and then
    /// failed, destroying the sign-in.
    #[test]
    fn a_failed_atomic_save_leaves_the_live_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.json");
        temp_env::with_var(ENV_ACCOUNT_FILE, Some(path.to_str().unwrap()), || {
            let stored = StoredAccount::from_secrets(&secrets());
            save_stored_account(&stored).unwrap();
            let before = std::fs::read(&path).unwrap();
            assert!(!before.is_empty(), "positive control: the sign-in exists");
            let mut other = stored.clone();
            other.access_token = "access-other".into();
            other.refresh_token = "refresh-other".into();
            FAIL_ATOMIC_FOR_TEST.store(true, std::sync::atomic::Ordering::SeqCst);
            let result = save_stored_account(&other);
            FAIL_ATOMIC_FOR_TEST.store(false, std::sync::atomic::Ordering::SeqCst);
            assert!(result.is_err(), "the full disk is reported");
            let after = std::fs::read(&path).unwrap();
            assert_eq!(
                before, after,
                "the live file is UNTOUCHED — the base behavior truncated it and then failed"
            );
        });
    }

    #[cfg(unix)]
    fn parent_pid_alive() -> u32 {
        std::os::unix::process::parent_id()
    }

    #[cfg(not(unix))]
    fn parent_pid_alive() -> u32 {
        1
    }

    /// N10: sign-out removes the atomic save's temporary files with the sign-in — they hold
    /// the same refresh token — and a save sweeps whatever an earlier crashed write left.
    #[test]
    fn sign_out_removes_the_write_temporary_files_and_a_save_sweeps_them() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.json");
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                (ENV_ACCOUNT_FILE, Some(path.to_str().unwrap())),
            ],
            || {
                let stored = StoredAccount::from_secrets(&secrets());
                save_stored_account(&stored).unwrap();
                // a crashed write's leftover, refresh token and all
                let leftover = dir.path().join("account.json.write-99999");
                std::fs::write(&leftover, b"refresh-secret").unwrap();
                clear_stored_account();
                assert!(!leftover.exists(), "sign-out removes the write temp");
                assert!(!path.exists(), "positive control: the sign-in is gone");
                // and the next save sweeps whatever appears again
                std::fs::write(&leftover, b"refresh-secret").unwrap();
                save_stored_account(&stored).unwrap();
                assert!(!leftover.exists(), "a save sweeps the write temp first");
                assert!(path.is_file(), "positive control: the save itself landed");
                // #6: a live foreign process's in-flight temp survives a save — the sweep is
                // pid-scoped. The parent pid is alive and never the pid the save stamps.
                let foreign = dir
                    .path()
                    .join(format!("account.json.write-{}", parent_pid_alive()));
                std::fs::write(&foreign, b"refresh-secret").unwrap();
                save_stored_account(&stored).unwrap();
                assert!(
                    foreign.exists(),
                    "a live foreign temp survives a save: the sweep is pid-scoped"
                );
                let _ = std::fs::remove_file(&foreign);
            },
        );
    }
}
