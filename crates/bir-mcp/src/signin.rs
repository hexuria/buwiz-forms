//! Desktop sign-in for `bir-mcp login`.
//!
//! The same OAuth device flow the Buwiz desktop app uses: this process asks
//! buwiz-server for a device code, the user approves it in the browser, and
//! the access and refresh tokens are stored in the OS keychain. The public
//! client id is `buwiz-desktop`. There is no client secret.
//!
//! The origin comes from the build. A debug build talks to the local
//! buwiz-server. Tests pass an origin in and an in-memory store, so they
//! never touch the keychain or a real server.

use crate::license::{UNAUTHENTICATED, UNLICENSED, interpret_desktop};
use serde_json::{Value, json};
use std::process::Command;
#[cfg(test)]
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Public OAuth client id. Not a secret.
pub const CLIENT_ID: &str = "buwiz-desktop";

const SERVICE: &str = "com.buwiz.bir-mcp";
const ACCOUNT: &str = "desktop-session";
const REFRESH_SKEW_SECS: i64 = 60;

/// Origin baked into this build. Debug builds use the local buwiz-server.
pub fn builtin_origin() -> &'static str {
    origin_for(cfg!(debug_assertions))
}

/// `true` is the debug origin. `false` is the release origin. Callers pick
/// one; this function does not contact either host.
pub fn origin_for(debug: bool) -> &'static str {
    if debug {
        "http://localhost:3008"
    } else {
        "https://api.buwiz.com"
    }
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
}

impl Session {
    fn to_json(&self) -> String {
        json!({
            "access_token": self.access_token,
            "refresh_token": self.refresh_token,
            "expires_at": self.expires_at,
        })
        .to_string()
    }

    fn parse(raw: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(raw).ok()?;
        let access_token = value.get("access_token")?.as_str()?.trim().to_string();
        let refresh_token = value.get("refresh_token")?.as_str()?.trim().to_string();
        let expires_at = value.get("expires_at")?.as_i64()?;
        if access_token.is_empty() || refresh_token.is_empty() {
            return None;
        }
        Some(Self {
            access_token,
            refresh_token,
            expires_at,
        })
    }

    pub(crate) fn from_token_body(body: &str, now: i64) -> Result<Self, String> {
        let value: Value = serde_json::from_str(body)
            .map_err(|_| "buwiz-server returned a sign-in bir-mcp could not read".to_string())?;
        let access_token = value
            .get("access_token")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let refresh_token = value
            .get("refresh_token")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if access_token.is_empty() || refresh_token.is_empty() {
            return Err("buwiz-server returned a sign-in bir-mcp could not store".to_string());
        }
        let expires_in = value
            .get("expires_in")
            .and_then(Value::as_i64)
            .unwrap_or(900)
            .max(0);
        Ok(Self {
            access_token,
            refresh_token,
            expires_at: now.saturating_add(expires_in),
        })
    }
}

pub trait SessionStore {
    fn load(&self) -> Result<Option<Session>, String>;
    fn save(&self, session: &Session) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
}

/// OS keychain entry `com.buwiz.bir-mcp` / `desktop-session`.
pub struct KeyringStore;

impl SessionStore for KeyringStore {
    fn load(&self) -> Result<Option<Session>, String> {
        let entry = keyring_entry()?;
        match entry.get_password() {
            Ok(raw) => match Session::parse(&raw) {
                Some(session) => Ok(Some(session)),
                None => {
                    let _ = entry.delete_credential();
                    Ok(None)
                }
            },
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(format!(
                "could not read the Buwiz sign-in from the keychain: {error}"
            )),
        }
    }

    fn save(&self, session: &Session) -> Result<(), String> {
        keyring_entry()?
            .set_password(&session.to_json())
            .map_err(|error| format!("could not store the Buwiz sign-in in the keychain: {error}"))
    }

    fn clear(&self) -> Result<(), String> {
        match keyring_entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!(
                "could not remove the Buwiz sign-in from the keychain: {error}"
            )),
        }
    }
}

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|error| format!("could not open the sign-in keychain: {error}"))
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

pub struct LoginHooks<F, N, S> {
    pub open_browser: F,
    pub now: N,
    pub sleep: S,
}

/// Run the device flow. The returned lines are safe to print: they contain
/// the user code and the device page, and they do not contain tokens.
pub fn login<F, N, S>(
    origin: &str,
    store: &impl SessionStore,
    mut hooks: LoginHooks<F, N, S>,
) -> Result<Vec<String>, String>
where
    F: FnMut(&str) -> Result<(), String>,
    N: FnMut() -> i64,
    S: FnMut(Duration),
{
    let origin = trim_origin(origin);
    let http = http_client()?;
    let started = start_device(&http, &origin)?;
    let mut lines = vec![
        "Sign in with Buwiz in your browser.".to_string(),
        format!("Code: {}", started.user_code),
        started.page.clone(),
    ];
    if let Err(error) = (hooks.open_browser)(&started.page) {
        lines.push(format!(
            "The browser did not open ({error}). Visit the page above and approve this computer."
        ));
    }
    lines.push("Waiting for approval...".to_string());
    let session = poll_until_approved(&http, &origin, &started, &mut hooks)?;
    store.save(&session)?;
    lines.push("Signed in.".to_string());
    Ok(lines)
}

struct DeviceStart {
    device_code: String,
    user_code: String,
    page: String,
    interval: i64,
    expires_in: i64,
}

fn start_device(http: &reqwest::blocking::Client, origin: &str) -> Result<DeviceStart, String> {
    let (status, body) = post_form(
        http,
        &format!("{origin}/oauth/device/code"),
        &form(&[("client_id", CLIENT_ID)]),
    )?;
    if status != 200 {
        return Err(format!(
            "buwiz-server answered HTTP {status} while starting sign-in"
        ));
    }
    let value: Value = serde_json::from_str(&body)
        .map_err(|_| "buwiz-server returned a device reply bir-mcp could not read".to_string())?;
    let device_code = value
        .get("device_code")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let user_code = value
        .get("user_code")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if device_code.is_empty() || user_code.is_empty() {
        return Err("buwiz-server did not return a device code".to_string());
    }
    let page = value
        .get("verification_uri_complete")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|page| !page.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let uri = value.get("verification_uri")?.as_str()?.trim();
            if uri.is_empty() {
                None
            } else {
                Some(format!("{uri}?user_code={}", form_encode(&user_code)))
            }
        })
        .ok_or_else(|| "buwiz-server did not return a device page".to_string())?;
    let interval = value
        .get("interval")
        .and_then(Value::as_i64)
        .filter(|seconds| *seconds >= 0)
        .unwrap_or(5);
    let expires_in = value
        .get("expires_in")
        .and_then(Value::as_i64)
        .filter(|seconds| *seconds > 0)
        .unwrap_or(900);
    Ok(DeviceStart {
        device_code,
        user_code,
        page,
        interval,
        expires_in,
    })
}

fn poll_until_approved<F, N, S>(
    http: &reqwest::blocking::Client,
    origin: &str,
    started: &DeviceStart,
    hooks: &mut LoginHooks<F, N, S>,
) -> Result<Session, String>
where
    F: FnMut(&str) -> Result<(), String>,
    N: FnMut() -> i64,
    S: FnMut(Duration),
{
    let mut interval = started.interval;
    let mut waited = 0i64;
    let budget = started.expires_in;
    loop {
        if waited >= budget {
            return Err("This sign-in code expired. Run `bir-mcp login` again.".to_string());
        }
        match poll_once(http, origin, &started.device_code, (hooks.now)())? {
            Poll::Ready(session) => return Ok(session),
            Poll::Pending => {
                (hooks.sleep)(Duration::from_secs(interval as u64));
                waited += interval.max(1);
            }
            Poll::SlowDown => {
                interval = (interval + 5).min(60);
                (hooks.sleep)(Duration::from_secs(interval as u64));
                waited += interval.max(1);
            }
        }
    }
}

enum Poll {
    Ready(Session),
    Pending,
    SlowDown,
}

fn poll_once(
    http: &reqwest::blocking::Client,
    origin: &str,
    device_code: &str,
    now: i64,
) -> Result<Poll, String> {
    let (status, body) = post_form(
        http,
        &format!("{origin}/oauth/token"),
        &form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", device_code),
            ("client_id", CLIENT_ID),
        ]),
    )?;
    if status == 200 {
        return Ok(Poll::Ready(Session::from_token_body(&body, now)?));
    }
    let code = oauth_error_code(&body);
    if status == 400 {
        return match code.as_deref() {
            Some("authorization_pending") => Ok(Poll::Pending),
            Some("slow_down") => Ok(Poll::SlowDown),
            Some("expired_token") => {
                Err("This sign-in code expired. Run `bir-mcp login` again.".to_string())
            }
            Some("access_denied") => {
                Err("Buwiz did not approve this computer. Run `bir-mcp login` again.".to_string())
            }
            Some(other) => Err(format!(
                "Buwiz did not complete sign-in ({other}). Run `bir-mcp login` again."
            )),
            None => Err("Buwiz did not complete sign-in. Run `bir-mcp login` again.".to_string()),
        };
    }
    Err(format!(
        "buwiz-server answered HTTP {status} during sign-in"
    ))
}

/// Exchange a desktop refresh token. The caller stores the new session.
pub(crate) fn exchange_refresh(origin: &str, refresh: &str, now: i64) -> Result<Session, String> {
    let origin = trim_origin(origin);
    let http = http_client()?;
    let (status, body) = post_form(
        &http,
        &format!("{origin}/oauth/token"),
        &form(&[("grant_type", "refresh_token"), ("refresh_token", refresh)]),
    )?;
    if status != 200 {
        return Err(format!(
            "buwiz-server answered HTTP {status} while refreshing sign-in"
        ));
    }
    Session::from_token_body(&body, now)
}

/// Load the stored session, refreshing when it expires within a minute.
/// `Ok(None)` means this computer is signed out. A failed refresh clears the
/// store and returns the sign-in sentence.
pub fn ensure_fresh(
    origin: &str,
    store: &impl SessionStore,
    now: i64,
) -> Result<Option<Session>, String> {
    let Some(session) = store.load()? else {
        return Ok(None);
    };
    if session.expires_at.saturating_sub(now) > REFRESH_SKEW_SECS {
        return Ok(Some(session));
    }
    match exchange_refresh(origin, &session.refresh_token, now) {
        Ok(fresh) => {
            store.save(&fresh)?;
            Ok(Some(fresh))
        }
        Err(_) => {
            store.clear()?;
            Err(UNAUTHENTICATED.to_string())
        }
    }
}

pub fn status(origin: &str, store: &impl SessionStore, now: i64) -> Result<String, String> {
    let Some(session) = ensure_fresh(origin, store, now)? else {
        return Ok("Not signed in. Run `bir-mcp login`.".to_string());
    };
    let origin = trim_origin(origin);
    let http = http_client()?;
    let (status, body) = account_body(&http, &origin, store, session, now)?;
    if status == 401 {
        return Ok("Not signed in. Run `bir-mcp login`.".to_string());
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
        let origin = trim_origin(origin);
        if let Ok(http) = http_client() {
            let _ = post_bearer_empty(
                &http,
                &format!("{origin}/api/auth/logout"),
                &session.access_token,
            );
        }
    }
    store.clear()?;
    Ok("Signed out.".to_string())
}

pub fn open_system_browser(url: &str) -> Result<(), String> {
    browser_command(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not open a browser: {error}"))
}

fn browser_command(url: &str) -> Command {
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg(url);
        return command;
    }
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        return command;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    }
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
        Err(_) => {
            store.clear()?;
            return Ok((401, String::new()));
        }
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

fn post_form(
    http: &reqwest::blocking::Client,
    url: &str,
    body: &str,
) -> Result<(u16, String), String> {
    let response = http
        .post(url)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("accept", "application/json")
        .body(body.to_string())
        .send()
        .map_err(|error| format!("could not reach buwiz-server at {url}: {error}"))?;
    read_response(response)
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
    read_response(response)
}

fn post_bearer_empty(
    http: &reqwest::blocking::Client,
    url: &str,
    token: &str,
) -> Result<(u16, String), String> {
    let response = http
        .post(url)
        .header("authorization", format!("Bearer {token}"))
        .header("accept", "application/json")
        .body("")
        .send()
        .map_err(|error| format!("could not reach buwiz-server at {url}: {error}"))?;
    read_response(response)
}

fn read_response(response: reqwest::blocking::Response) -> Result<(u16, String), String> {
    let status = response.status().as_u16();
    let body = response.text().map_err(|error| {
        format!("buwiz-server closed the sign-in request before a body arrived: {error}")
    })?;
    Ok((status, body))
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{key}={}", form_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn form_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn oauth_error_code(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let code = value.get("error")?.as_str()?.trim();
    if code.is_empty() || !code.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_') {
        return None;
    }
    Some(code.to_string())
}

fn trim_origin(origin: &str) -> String {
    origin.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Script {
        device_status: u16,
        device_body: String,
        polls: Mutex<Vec<(u16, String)>>,
        seen: Mutex<Vec<(String, String)>>,
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
                script
                    .seen
                    .lock()
                    .expect("seen")
                    .push((request.url().to_string(), incoming.clone()));
                let (status, body) = if request.url() == "/oauth/device/code" {
                    (script.device_status, script.device_body.clone())
                } else if request.url() == "/oauth/token" {
                    script
                        .polls
                        .lock()
                        .expect("polls")
                        .pop()
                        .unwrap_or((500, "missing poll".to_string()))
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
            "verification_uri": "http://localhost:3008/login/device",
            "verification_uri_complete": "http://localhost:3008/login/device?user_code=ABCD-EFGH",
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

    #[test]
    fn origins_come_from_the_build_and_are_not_contacted_here() {
        assert_eq!(origin_for(true), "http://localhost:3008");
        assert_eq!(origin_for(false), "https://api.buwiz.com");
        assert_eq!(CLIENT_ID, "buwiz-desktop");
    }

    #[test]
    fn login_stores_the_session_after_the_user_approves_and_hides_tokens() {
        let origin = serve(Script {
            device_status: 200,
            device_body: device_body(0),
            polls: Mutex::new(vec![
                (200, token_body("access-secret", "refresh-secret")),
                (
                    400,
                    r#"{"error":"authorization_pending","error_description":"wait"}"#.to_string(),
                ),
            ]),
            seen: Mutex::new(Vec::new()),
        });
        let opened = Mutex::new(Vec::new());
        let slept = Mutex::new(Vec::new());
        let store = MemoryStore::default();
        let lines = login(
            &origin,
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
            },
        )
        .unwrap();
        let visible = lines.join("\n");
        assert!(visible.contains("ABCD-EFGH"), "{visible}");
        assert!(
            visible.contains("http://localhost:3008/login/device?user_code=ABCD-EFGH"),
            "{visible}"
        );
        assert!(visible.contains("Signed in."), "{visible}");
        assert!(!visible.contains("access-secret"), "{visible}");
        assert!(!visible.contains("refresh-secret"), "{visible}");
        assert!(!visible.contains("device-secret"), "{visible}");
        assert_eq!(
            opened.lock().unwrap().as_slice(),
            ["http://localhost:3008/login/device?user_code=ABCD-EFGH"]
        );
        assert_eq!(slept.lock().unwrap().as_slice(), [0]);
        let saved = store.load().unwrap().unwrap();
        assert_eq!(saved.access_token, "access-secret");
        assert_eq!(saved.refresh_token, "refresh-secret");
        assert_eq!(saved.expires_at, 1_700_000_000 + 900);
    }

    #[test]
    fn a_closed_browser_still_waits_for_approval() {
        let origin = serve(Script {
            device_status: 200,
            device_body: device_body(0),
            polls: Mutex::new(vec![(200, token_body("access-secret", "refresh-secret"))]),
            seen: Mutex::new(Vec::new()),
        });
        let store = MemoryStore::default();
        let lines = login(
            &origin,
            &store,
            LoginHooks {
                open_browser: |_url: &str| Err("no display".to_string()),
                now: || 10,
                sleep: |_duration| {},
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
        let origin = serve(Script {
            device_status: 200,
            device_body: device_body(0),
            polls: Mutex::new(vec![
                (200, token_body("access-secret", "refresh-secret")),
                (400, r#"{"error":"slow_down"}"#.to_string()),
            ]),
            seen: Mutex::new(Vec::new()),
        });
        let slept = Mutex::new(Vec::new());
        let store = MemoryStore::default();
        login(
            &origin,
            &store,
            LoginHooks {
                open_browser: |_url: &str| Ok(()),
                now: || 10,
                sleep: |duration: Duration| {
                    slept.lock().unwrap().push(duration.as_secs());
                },
            },
        )
        .unwrap();
        assert_eq!(slept.lock().unwrap().as_slice(), [5]);

        let origin = serve(Script {
            device_status: 200,
            device_body: device_body(0),
            polls: Mutex::new(vec![(400, r#"{"error":"access_denied"}"#.to_string())]),
            seen: Mutex::new(Vec::new()),
        });
        let store = MemoryStore::default();
        let error = login(
            &origin,
            &store,
            LoginHooks {
                open_browser: |_url: &str| Ok(()),
                now: || 10,
                sleep: |_duration| {},
            },
        )
        .unwrap_err();
        assert!(error.contains("did not approve"), "{error}");
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn refresh_replaces_the_stored_access_token_and_a_failure_clears_it() {
        let origin = serve(Script {
            device_status: 200,
            device_body: device_body(0),
            polls: Mutex::new(vec![(200, token_body("new-access", "new-refresh"))]),
            seen: Mutex::new(Vec::new()),
        });
        let store = MemoryStore::default();
        store
            .save(&Session {
                access_token: "old-access".to_string(),
                refresh_token: "old-refresh".to_string(),
                expires_at: 50,
            })
            .unwrap();
        let fresh = ensure_fresh(&origin, &store, 10).unwrap().unwrap();
        assert_eq!(fresh.access_token, "new-access");
        assert_eq!(fresh.refresh_token, "new-refresh");
        assert_eq!(store.load().unwrap().unwrap().access_token, "new-access");

        let store = MemoryStore::default();
        store
            .save(&Session {
                access_token: "still-good".to_string(),
                refresh_token: "keep".to_string(),
                expires_at: 10_000,
            })
            .unwrap();
        let kept = ensure_fresh("http://127.0.0.1:9", &store, 10)
            .unwrap()
            .unwrap();
        assert_eq!(kept.access_token, "still-good");

        let origin = serve(Script {
            device_status: 200,
            device_body: device_body(0),
            polls: Mutex::new(vec![(400, r#"{"error":"invalid_grant"}"#.to_string())]),
            seen: Mutex::new(Vec::new()),
        });
        let store = MemoryStore::default();
        store
            .save(&Session {
                access_token: "old-access".to_string(),
                refresh_token: "old-refresh".to_string(),
                expires_at: 10,
            })
            .unwrap();
        let error = ensure_fresh(&origin, &store, 10).unwrap_err();
        assert_eq!(error, UNAUTHENTICATED);
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn status_names_the_account_and_logout_clears_the_store() {
        let origin = serve(Script {
            device_status: 200,
            device_body: device_body(0),
            polls: Mutex::new(Vec::new()),
            seen: Mutex::new(Vec::new()),
        });
        let store = MemoryStore::default();
        assert_eq!(
            status(&origin, &store, 10).unwrap(),
            "Not signed in. Run `bir-mcp login`."
        );
        store
            .save(&Session {
                access_token: "access-secret".to_string(),
                refresh_token: "refresh-secret".to_string(),
                expires_at: 10_000,
            })
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

    #[test]
    fn browser_command_uses_the_platform_opener() {
        let command = browser_command("http://localhost:3008/login/device?user_code=ABCD");
        #[cfg(target_os = "linux")]
        assert_eq!(command.get_program(), "xdg-open");
        #[cfg(target_os = "macos")]
        assert_eq!(command.get_program(), "open");
        #[cfg(target_os = "windows")]
        assert_eq!(command.get_program(), "cmd");
    }
}
