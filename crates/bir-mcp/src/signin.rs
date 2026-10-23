//! Desktop sign-in for `bir-mcp login`.
//!
//! This is the same device flow and the same saved account the Buwiz app
//! uses (`bir_core::cloud_account`). The public client id is `buwiz-desktop`.
//! There is no client secret, and there is no second keychain.
//!
//! The origin comes from the build. A debug build talks to the local
//! buwiz-server. Tests pass an origin in and an in-memory store, so they
//! never touch the keychain or a real server.

use crate::license::{UNAUTHENTICATED, UNLICENSED, interpret_desktop};
use bir_core::cloud_account::{
    self, AccountBrowser, BlockingHttp, CloudError, StoredAccount, TokenSet,
};
use serde_json::Value;
#[cfg(test)]
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Public OAuth client id. Not a secret. Same value the desktop app sends.
pub const CLIENT_ID: &str = cloud_account::DESKTOP_CLIENT_ID;

const REFRESH_SKEW_SECS: i64 = 60;

const SIGNED_OUT: &str = "Not signed in. Sign in from the Buwiz app, or run `bir-mcp login`.";

/// Origin baked into this build. Debug builds use the local buwiz-server.
pub fn builtin_origin() -> &'static str {
    origin_for(cfg!(debug_assertions))
}

/// `true` is the debug origin. `false` is the release origin. Callers pick
/// one; this function does not contact either host.
pub fn origin_for(debug: bool) -> &'static str {
    if debug {
        cloud_account::DEV_SERVER_URL
    } else {
        cloud_account::PRODUCTION_SERVER_URL
    }
}

/// API origin and account-page origin, using the desktop app's precedence
/// (`BUWIZ_SERVER_URL` / `BUWIZ_ACCOUNT_URL`, then the build default).
pub fn configured_origins() -> Result<(String, String), String> {
    let server = cloud_account::configured_server().map_err(|error| error.to_string())?;
    let account = cloud_account::configured_account().map_err(|error| error.to_string())?;
    Ok((server.url, account.url))
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Access token, refresh token, and absolute expiry (unix seconds).
/// Never log this value.
/// `Debug` exists so tests can unwrap results. The binary never prints a session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub server_url: String,
    pub user_id: String,
    pub email: Option<String>,
}

impl Session {
    fn from_stored(stored: &StoredAccount) -> Self {
        Self {
            access_token: stored.access_token.clone(),
            refresh_token: stored.refresh_token.clone(),
            expires_at: stored.access_expires_at_unix,
            server_url: stored.server_url.clone(),
            user_id: stored.user_id.clone(),
            email: stored.email.clone(),
        }
    }

    fn to_stored(&self) -> StoredAccount {
        StoredAccount {
            server_url: self.server_url.clone(),
            access_token: self.access_token.clone(),
            refresh_token: self.refresh_token.clone(),
            access_expires_at_unix: self.expires_at,
            user_id: self.user_id.clone(),
            email: self.email.clone(),
            offline: false,
            last_refresh_unix: unix_now(),
        }
    }
}

pub trait SessionStore {
    fn load(&self) -> Result<Option<Session>, String>;
    fn save(&self, session: &Session) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
}

/// The account store the Buwiz app writes: keychain `com.buwiz.bir` /
/// `desktop-refresh`, and `buwiz-cloud-account.json` beside the database.
pub struct DesktopStore;

impl SessionStore for DesktopStore {
    fn load(&self) -> Result<Option<Session>, String> {
        match cloud_account::load_stored_account() {
            Ok(Some(stored)) => Ok(Some(Session::from_stored(&stored))),
            Ok(None) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn save(&self, session: &Session) -> Result<(), String> {
        let mut stored = match cloud_account::load_stored_account() {
            Ok(Some(stored)) => stored,
            Ok(None) => session.to_stored(),
            Err(error) => return Err(error.to_string()),
        };
        if !session.server_url.is_empty() {
            stored.server_url = session.server_url.clone();
        }
        stored.access_token = session.access_token.clone();
        stored.refresh_token = session.refresh_token.clone();
        stored.access_expires_at_unix = session.expires_at;
        if !session.user_id.is_empty() {
            stored.user_id = session.user_id.clone();
        }
        if session.email.is_some() {
            stored.email = session.email.clone();
        }
        stored.offline = false;
        stored.last_refresh_unix = unix_now();
        cloud_account::save_stored_account(&stored).map_err(|error| error.to_string())
    }

    fn clear(&self) -> Result<(), String> {
        cloud_account::clear_stored_account();
        Ok(())
    }
}

/// Write a refreshed access token back to the desktop account.
/// An existing user id and email stay when the refresh response has none.
pub fn persist_refresh(
    origin: &str,
    access: &str,
    refresh: &str,
    expires_at: i64,
) -> Result<(), String> {
    DesktopStore.save(&Session {
        access_token: access.to_string(),
        refresh_token: refresh.to_string(),
        expires_at,
        server_url: trim_origin(origin),
        user_id: String::new(),
        email: None,
    })
}

/// Drop the desktop account from the keychain and the private file.
pub fn clear_desktop_session() {
    cloud_account::clear_stored_account();
}

#[cfg(test)]
#[derive(Default)]
struct MemoryStore {
    inner: Mutex<Option<Session>>,
}

#[cfg(test)]
impl SessionStore for MemoryStore {
    fn load(&self) -> Result<Option<Session>, String> {
        Ok(self.inner.lock().expect("session store").clone())
    }

    fn save(&self, session: &Session) -> Result<(), String> {
        *self.inner.lock().expect("session store") = Some(session.clone());
        Ok(())
    }

    fn clear(&self) -> Result<(), String> {
        *self.inner.lock().expect("session store") = None;
        Ok(())
    }
}

pub struct LoginHooks<F, N, S, E> {
    pub open_browser: F,
    pub now: N,
    pub sleep: S,
    /// Called with each user-visible line as it happens, before the poll ends.
    pub emit: E,
}

/// Run the desktop device flow. The returned lines are safe to print: they
/// contain the user code and the device page, and they do not contain tokens.
///
/// `api_origin` is where token calls go. `account_origin` is the browser
/// origin. A debug build uses the same host for both.
pub fn login<F, N, S, E>(
    api_origin: &str,
    account_origin: &str,
    store: &impl SessionStore,
    mut hooks: LoginHooks<F, N, S, E>,
) -> Result<Vec<String>, String>
where
    F: FnMut(&str) -> Result<(), String>,
    N: FnMut() -> i64,
    S: FnMut(Duration),
    E: FnMut(&str),
{
    let api_origin = trim_origin(api_origin);
    let account_origin = trim_origin(account_origin);
    let mut http = BlockingHttp::new();
    let started_at = (hooks.now)();
    let challenge =
        cloud_account::start_device(&mut http, &api_origin, started_at).map_err(login_error)?;
    let page = cloud_account::account_browser_url(
        &account_origin,
        AccountBrowser::SignIn,
        &challenge.user_code,
    );
    let mut lines = Vec::new();
    emit_line(
        &mut hooks,
        &mut lines,
        "Sign in with Buwiz in your browser.".to_string(),
    );
    emit_line(
        &mut hooks,
        &mut lines,
        format!("Code: {}", challenge.user_code),
    );
    emit_line(&mut hooks, &mut lines, page.clone());
    if let Err(error) = (hooks.open_browser)(&page) {
        emit_line(
            &mut hooks,
            &mut lines,
            format!(
                "The browser did not open ({error}). Visit the page above and approve this computer."
            ),
        );
    }
    emit_line(
        &mut hooks,
        &mut lines,
        "Waiting for approval...".to_string(),
    );
    let tokens = {
        let LoginHooks { now, sleep, .. } = &mut hooks;
        cloud_account::poll_until(&mut http, &challenge, now, sleep).map_err(login_error)?
    };
    if tokens.refresh_token.is_empty() {
        return Err("The server did not return a refresh token. Sign in again.".to_string());
    }
    let (user_id, email) =
        cloud_account::load_profile(&mut http, &api_origin, &tokens.access_token)
            .map_err(login_error)?;
    let now = (hooks.now)();
    store.save(&session_from_tokens(
        &api_origin,
        tokens,
        now,
        user_id,
        email,
    ))?;
    emit_line(&mut hooks, &mut lines, "Signed in.".to_string());
    Ok(lines)
}

fn emit_line<F, N, S, E>(hooks: &mut LoginHooks<F, N, S, E>, lines: &mut Vec<String>, line: String)
where
    E: FnMut(&str),
{
    (hooks.emit)(&line);
    lines.push(line);
}

fn session_from_tokens(
    api_origin: &str,
    tokens: TokenSet,
    now: i64,
    user_id: String,
    email: Option<String>,
) -> Session {
    Session {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_at: now.saturating_add(tokens.expires_in.max(1)),
        server_url: api_origin.to_string(),
        user_id,
        email,
    }
}

fn login_error(error: CloudError) -> String {
    match error {
        CloudError::TimedOut => "This sign-in code expired. Run `bir-mcp login` again.".to_string(),
        CloudError::Server { description }
            if description == "expired_token"
                || description.to_ascii_lowercase().contains("expired") =>
        {
            "This sign-in code expired. Run `bir-mcp login` again.".to_string()
        }
        CloudError::Server { description }
            if description == "access_denied"
                || description.to_ascii_lowercase().contains("denied") =>
        {
            "Buwiz did not approve this computer. Run `bir-mcp login` again.".to_string()
        }
        CloudError::InvalidGrant => {
            "Buwiz did not approve this computer. Run `bir-mcp login` again.".to_string()
        }
        other => format!("{other} Run `bir-mcp login` again."),
    }
}

/// Why a refresh did not produce a new access token.
#[derive(Debug)]
pub(crate) enum RefreshError {
    /// The server rejected the refresh token. The caller deletes the account.
    Rejected,
    /// The account is still stored. The tool cannot run until the server answers.
    Other(String),
}

/// Exchange a desktop refresh token. The caller stores the new session.
pub(crate) fn exchange_refresh(
    origin: &str,
    refresh: &str,
    now: i64,
) -> Result<Session, RefreshError> {
    let origin = trim_origin(origin);
    let mut http = BlockingHttp::new();
    match cloud_account::refresh_tokens(&mut http, &origin, refresh) {
        Ok(tokens) => Ok(Session {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_at: now.saturating_add(tokens.expires_in.max(1)),
            server_url: origin,
            user_id: String::new(),
            email: None,
        }),
        Err(CloudError::InvalidGrant) => Err(RefreshError::Rejected),
        Err(CloudError::Network(message)) => Err(RefreshError::Other(format!(
            "could not reach buwiz-server at {origin} to refresh sign-in: {message}"
        ))),
        Err(error) => Err(RefreshError::Other(format!(
            "could not refresh the Buwiz sign-in: {error}"
        ))),
    }
}

/// Load the stored session, refreshing when it expires within a minute.
/// `Ok(None)` means this computer is signed out. A rejected refresh clears
/// the store. A network failure leaves the store in place.
pub fn ensure_fresh(
    origin: &str,
    store: &impl SessionStore,
    now: i64,
) -> Result<Option<Session>, String> {
    let Some(session) = store.load()? else {
        return Ok(None);
    };
    let origin = trim_origin(origin);
    if !session.server_url.is_empty() && session.server_url != origin {
        store.clear()?;
        return Ok(None);
    }
    if session.expires_at.saturating_sub(now) > REFRESH_SKEW_SECS {
        return Ok(Some(session));
    }
    match exchange_refresh(&origin, &session.refresh_token, now) {
        Ok(mut fresh) => {
            fresh.user_id = session.user_id;
            fresh.email = session.email;
            if fresh.server_url.is_empty() {
                fresh.server_url = origin;
            }
            store.save(&fresh)?;
            Ok(Some(fresh))
        }
        Err(RefreshError::Rejected) => {
            store.clear()?;
            Err(UNAUTHENTICATED.to_string())
        }
        Err(RefreshError::Other(message)) => Err(message),
    }
}

pub fn status(origin: &str, store: &impl SessionStore, now: i64) -> Result<String, String> {
    let Some(session) = ensure_fresh(origin, store, now)? else {
        return Ok(SIGNED_OUT.to_string());
    };
    let origin = trim_origin(origin);
    let http = http_client()?;
    let (status, body) = account_body(&http, &origin, store, session, now)?;
    if status == 401 {
        return Ok(SIGNED_OUT.to_string());
    }
    if status != 200 {
        return Err(format!(
            "buwiz-server answered HTTP {status} while reading the signed-in account"
        ));
    }
    let email = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("email")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|email| !email.is_empty());
    let who = match email {
        Some(email) => format!("Signed in as {email}."),
        None => "Signed in.".to_string(),
    };
    let access_token = store
        .load()?
        .map(|session| session.access_token)
        .unwrap_or_default();
    let (profile_status, profile_body) =
        get_bearer(&http, &format!("{origin}/api/tax-profiles"), &access_token)?;
    let access = match interpret_desktop(profile_status, &profile_body) {
        Ok(()) => "A taxpayer profile is available.",
        Err(message) if message == UNLICENSED => "No taxpayer profile has a working pass yet.",
        Err(message) => return Err(message),
    };
    Ok(format!("{who} {access}"))
}

pub fn logout(origin: &str, store: &impl SessionStore) -> Result<String, String> {
    if let Some(session) = store.load()? {
        let mut http = BlockingHttp::new();
        let mut secrets = session.to_stored().to_secrets();
        if secrets.server_url.is_empty() {
            secrets.server_url = trim_origin(origin);
        }
        cloud_account::revoke_session(&mut http, &secrets);
    }
    store.clear()?;
    Ok("Signed out.".to_string())
}

pub fn open_system_browser(url: &str) -> Result<(), String> {
    cloud_account::open_browser(url)
}

fn account_body(
    http: &reqwest::blocking::Client,
    origin: &str,
    store: &impl SessionStore,
    session: Session,
    now: i64,
) -> Result<(u16, String), String> {
    let (status, body) = get_bearer(http, &format!("{origin}/me"), &session.access_token)?;
    if status != 401 {
        return Ok((status, body));
    }
    let fresh = match exchange_refresh(origin, &session.refresh_token, now) {
        Ok(fresh) => fresh,
        Err(RefreshError::Rejected) => {
            store.clear()?;
            return Ok((401, String::new()));
        }
        Err(RefreshError::Other(message)) => return Err(message),
    };
    store.save(&fresh)?;
    let (status, body) = get_bearer(http, &format!("{origin}/me"), &fresh.access_token)?;
    if status == 401 {
        store.clear()?;
    }
    Ok((status, body))
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("could not prepare the Buwiz sign-in request: {error}"))
}

fn get_bearer(
    http: &reqwest::blocking::Client,
    url: &str,
    token: &str,
) -> Result<(u16, String), String> {
    let response = http
        .get(url)
        .header("authorization", format!("Bearer {token}"))
        .header("accept", "application/json")
        .send()
        .map_err(|error| format!("could not reach buwiz-server at {url}: {error}"))?;
    let status = response.status().as_u16();
    let body = response.text().map_err(|error| {
        format!("buwiz-server closed the sign-in request before a body arrived: {error}")
    })?;
    Ok((status, body))
}

fn trim_origin(origin: &str) -> String {
    origin.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Script {
        device_status: u16,
        device_body: String,
        polls: Mutex<Vec<(u16, String)>>,
        refreshes: Mutex<Vec<(u16, String)>>,
    }

    fn serve(script: Script) -> String {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_ip().unwrap();
        std::thread::spawn(move || {
            loop {
                let mut request = match server.recv_timeout(Duration::from_secs(5)) {
                    Ok(Some(request)) => request,
                    _ => break,
                };
                let mut incoming = String::new();
                std::io::Read::read_to_string(request.as_reader(), &mut incoming).unwrap();
                let (status, body) = if request.url() == "/auth/device/start" {
                    (script.device_status, script.device_body.clone())
                } else if request.url() == "/auth/device/poll" {
                    script
                        .polls
                        .lock()
                        .expect("polls")
                        .pop()
                        .unwrap_or((500, "missing poll".to_string()))
                } else if request.url() == "/oauth/token" {
                    script
                        .refreshes
                        .lock()
                        .expect("refreshes")
                        .pop()
                        .unwrap_or((500, "missing refresh".to_string()))
                } else if request.url() == "/me" {
                    (
                        200,
                        r#"{"user_id":"u","email":"owner@example.test","orgs":[]}"#.to_string(),
                    )
                } else if request.url() == "/api/tax-profiles" {
                    (
                        200,
                        r#"{"profiles":[{"permissions":["profile.view"]}]}"#.to_string(),
                    )
                } else if request.url() == "/api/auth/logout" {
                    (204, String::new())
                } else {
                    (404, "nope".to_string())
                };
                let response = tiny_http::Response::from_string(body).with_status_code(status);
                let _ = request.respond(response);
            }
        });
        format!("http://{addr}")
    }

    fn device_body(interval: i64) -> String {
        json!({
            "device_code": "device-secret",
            "user_code": "ABCD-EFGH",
            "verification_uri": "http://localhost:3008/device",
            "verification_uri_complete": "http://localhost:3008/device?user_code=ABCD-EFGH",
            "expires_in": 30,
            "interval": interval,
        })
        .to_string()
    }

    fn token_body(access: &str, refresh: &str) -> String {
        json!({
            "access_token": access,
            "refresh_token": refresh,
            "token_type": "Bearer",
            "expires_in": 900,
        })
        .to_string()
    }

    fn script(
        device_body: String,
        polls: Vec<(u16, String)>,
        refreshes: Vec<(u16, String)>,
    ) -> Script {
        Script {
            device_status: 200,
            device_body,
            polls: Mutex::new(polls),
            refreshes: Mutex::new(refreshes),
        }
    }

    fn bare(access_token: &str, refresh_token: &str, expires_at: i64) -> Session {
        Session {
            access_token: access_token.to_string(),
            refresh_token: refresh_token.to_string(),
            expires_at,
            server_url: String::new(),
            user_id: String::new(),
            email: None,
        }
    }

    #[test]
    fn origins_come_from_the_build_and_are_not_contacted_here() {
        assert_eq!(origin_for(true), cloud_account::DEV_SERVER_URL);
        assert_eq!(origin_for(false), cloud_account::PRODUCTION_SERVER_URL);
        assert_eq!(CLIENT_ID, "buwiz-desktop");
        assert_eq!(
            cloud_account::account_browser_url(
                "http://localhost:3008",
                AccountBrowser::SignIn,
                "ABCD-EFGH"
            ),
            "http://localhost:3008/login?next=%2Fdevice%3Fuser_code%3DABCD-EFGH"
        );
    }

    #[test]
    fn login_stores_the_session_after_the_user_approves_and_hides_tokens() {
        let origin = serve(script(
            device_body(0),
            vec![
                (200, token_body("access-secret", "refresh-secret")),
                (
                    400,
                    r#"{"error":"authorization_pending","error_description":"wait"}"#.to_string(),
                ),
            ],
            Vec::new(),
        ));
        let opened = Mutex::new(Vec::new());
        let slept = Mutex::new(Vec::new());
        let store = MemoryStore::default();
        let lines = login(
            &origin,
            "http://localhost:3008",
            &store,
            LoginHooks {
                open_browser: |url: &str| {
                    opened.lock().expect("opened").push(url.to_string());
                    Ok(())
                },
                now: || 1_700_000_000,
                sleep: |duration: Duration| {
                    slept.lock().expect("slept").push(duration.as_secs());
                },
                emit: |_: &str| {},
            },
        )
        .unwrap();
        let visible = lines.join("\n");
        assert!(visible.contains("ABCD-EFGH"), "{visible}");
        assert!(
            visible.contains("http://localhost:3008/login?next=%2Fdevice%3Fuser_code%3DABCD-EFGH"),
            "{visible}"
        );
        assert!(visible.contains("Signed in."), "{visible}");
        assert!(!visible.contains("access-secret"), "{visible}");
        assert!(!visible.contains("refresh-secret"), "{visible}");
        assert!(!visible.contains("device-secret"), "{visible}");
        assert_eq!(
            opened.lock().unwrap().as_slice(),
            ["http://localhost:3008/login?next=%2Fdevice%3Fuser_code%3DABCD-EFGH"]
        );
        assert_eq!(slept.lock().unwrap().as_slice(), [1]);
        let saved = store.load().unwrap().unwrap();
        assert_eq!(saved.access_token, "access-secret");
        assert_eq!(saved.refresh_token, "refresh-secret");
        assert_eq!(saved.expires_at, 1_700_000_000 + 900);
        assert_eq!(saved.server_url, origin);
        assert_eq!(saved.user_id, "u");
        assert_eq!(saved.email.as_deref(), Some("owner@example.test"));
    }

    #[test]
    fn a_closed_browser_still_waits_for_approval() {
        let origin = serve(script(
            device_body(0),
            vec![(200, token_body("access-secret", "refresh-secret"))],
            Vec::new(),
        ));
        let store = MemoryStore::default();
        let lines = login(
            &origin,
            "http://localhost:3008",
            &store,
            LoginHooks {
                open_browser: |_url: &str| Err("no display".to_string()),
                now: || 10,
                sleep: |_duration| {},
                emit: |_: &str| {},
            },
        )
        .unwrap();
        let visible = lines.join("\n");
        assert!(visible.contains("The browser did not open"), "{visible}");
        assert!(visible.contains("Signed in."), "{visible}");
        assert!(store.load().unwrap().is_some());
    }

    #[test]
    fn slow_down_waits_longer_and_a_denial_stores_nothing() {
        let origin = serve(script(
            device_body(5),
            vec![
                (200, token_body("access-secret", "refresh-secret")),
                (400, r#"{"error":"slow_down"}"#.to_string()),
            ],
            Vec::new(),
        ));
        let slept = Mutex::new(Vec::new());
        let store = MemoryStore::default();
        login(
            &origin,
            "http://localhost:3008",
            &store,
            LoginHooks {
                open_browser: |_url: &str| Ok(()),
                now: || 10,
                sleep: |duration: Duration| {
                    slept.lock().unwrap().push(duration.as_secs());
                },
                emit: |_: &str| {},
            },
        )
        .unwrap();
        assert_eq!(slept.lock().unwrap().as_slice(), [10]);

        let origin = serve(script(
            device_body(0),
            vec![(400, r#"{"error":"access_denied"}"#.to_string())],
            Vec::new(),
        ));
        let store = MemoryStore::default();
        let error = login(
            &origin,
            "http://localhost:3008",
            &store,
            LoginHooks {
                open_browser: |_url: &str| Ok(()),
                now: || 10,
                sleep: |_duration| {},
                emit: |_: &str| {},
            },
        )
        .unwrap_err();
        assert!(error.contains("did not approve"), "{error}");
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn refresh_replaces_the_stored_access_token_and_a_failure_clears_it() {
        let origin = serve(script(
            device_body(0),
            Vec::new(),
            vec![(200, token_body("new-access", "new-refresh"))],
        ));
        let store = MemoryStore::default();
        store.save(&bare("old-access", "old-refresh", 50)).unwrap();
        let fresh = ensure_fresh(&origin, &store, 10).unwrap().unwrap();
        assert_eq!(fresh.access_token, "new-access");
        assert_eq!(fresh.refresh_token, "new-refresh");
        assert_eq!(store.load().unwrap().unwrap().access_token, "new-access");

        let store = MemoryStore::default();
        store.save(&bare("still-good", "keep", 10_000)).unwrap();
        let kept = ensure_fresh("http://127.0.0.1:9", &store, 10)
            .unwrap()
            .unwrap();
        assert_eq!(kept.access_token, "still-good");

        let origin = serve(script(
            device_body(0),
            Vec::new(),
            vec![(400, r#"{"error":"invalid_grant"}"#.to_string())],
        ));
        let store = MemoryStore::default();
        store.save(&bare("old-access", "old-refresh", 10)).unwrap();
        let error = ensure_fresh(&origin, &store, 10).unwrap_err();
        assert_eq!(error, UNAUTHENTICATED);
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn a_session_for_a_different_server_is_dropped() {
        let store = MemoryStore::default();
        let mut session = bare("access-secret", "refresh-secret", 10_000);
        session.server_url = "https://api.buwiz.com".to_string();
        store.save(&session).unwrap();
        let fresh = ensure_fresh("http://localhost:3008", &store, 10).unwrap();
        assert!(fresh.is_none());
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn a_refresh_the_server_never_answers_keeps_the_account() {
        let store = MemoryStore::default();
        store.save(&bare("old-access", "old-refresh", 10)).unwrap();
        let error = ensure_fresh("http://127.0.0.1:1", &store, 10).unwrap_err();
        assert!(error.contains("could not reach buwiz-server"), "{error}");
        assert_eq!(store.load().unwrap().unwrap().refresh_token, "old-refresh");
    }

    #[test]
    fn status_names_the_account_and_logout_clears_the_store() {
        let origin = serve(script(device_body(0), Vec::new(), Vec::new()));
        let store = MemoryStore::default();
        assert_eq!(status(&origin, &store, 10).unwrap(), SIGNED_OUT);
        store
            .save(&bare("access-secret", "refresh-secret", 10_000))
            .unwrap();
        let line = status(&origin, &store, 10).unwrap();
        assert_eq!(
            line,
            "Signed in as owner@example.test. A taxpayer profile is available."
        );
        assert!(!line.contains("access-secret"), "{line}");
        assert_eq!(logout(&origin, &store).unwrap(), "Signed out.");
        assert!(store.load().unwrap().is_none());
        assert_eq!(logout(&origin, &store).unwrap(), "Signed out.");
    }
}
