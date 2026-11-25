//! In-memory account server for development. Uses the same small Tokio HTTP stack as the mock relay.
//!
//! It speaks the device flow (`/auth/device/*`), the email-and-code sign-in (`/auth/email/start`, `/resend`, `/verify`),
//! the refresh, `/me`, logout, and the device registration (`POST /api/devices`). Every answer is the contract's;
//! nothing is stored once it exits.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(900);
const DEFAULT_EMAIL: &str = "dev@buwiz.local";
/// The first email sign-in's code unless the options say otherwise; each later code (a new request or a Resend) counts
/// up from it: 246810, 246811, … Every code is printed when it is sent.
pub const DEFAULT_CODE: &str = "246810";
/// Wrong codes a request takes before it answers `too_many_attempts`.
const ATTEMPTS: u32 = 5;
/// Codes mailed to one address in an hour (start and resend together) before `too_many_requests` (the contract's shared
/// per-address limit).
const PER_ADDRESS_HOURLY: usize = 5;
/// Start and resend calls one network may make in an hour, answered or not, before `too_many_requests` (#81: 200).
/// The real server also shares one hourly budget among all networks past 100,000 in an hour; it answers the same
/// `too_many_requests`, so the mock does not model it.
pub const START_PER_NETWORK_HOURLY: usize = 200;
/// Verify calls one network may make in an hour, app and web together, before `too_many_requests` (#81: 300; none of
/// them spends a request's try).
pub const VERIFY_PER_NETWORK_HOURLY: usize = 300;
/// The clients the email sign-in serves (a `CLIENTS` entry with scope `app`): the contract's, and the one this app sends.
const APP_CLIENTS: [&str; 2] = ["buwiz-ebirforms", "buwiz-desktop"];
const PLATFORMS: [&str; 6] = ["ios", "ipados", "android", "macos", "windows", "linux"];

/// How the mock answers the email sign-in.
#[derive(Clone, Debug)]
pub struct MockOptions {
    /// Approve device codes at once as [`DEFAULT_EMAIL`].
    pub auto_approve: bool,
    /// The first code; each later one (a new request or a Resend) counts up from it, so no two requests share one.
    /// `None`: a new random code each time. Either way it is printed, and [`MockAccount::code_for`] tells a test.
    pub code: Option<String>,
    /// How long a code works (`expires_in`).
    pub code_ttl: Duration,
    /// How long until Resend is allowed (`resend_after`); the contract makes it the code's life.
    pub resend_wait: Duration,
    /// Every code is wrong (to see the wrong-code answer).
    pub wrong_codes: bool,
    /// Every code has expired by the time it is checked.
    pub expired_codes: bool,
    /// The email sign-in is not offered: `/auth/email/*` answers 404 (the app falls back to the browser).
    pub no_email: bool,
    /// Addresses whose accounts are banned (`account_banned` at verify).
    pub banned: Vec<String>,
    /// `/auth/email/*` answer a challenge page (HTML, 200), as a rule in front of a server can.
    pub html: bool,
    /// `/auth/email/*` answer the outer rate limit's HTML 429.
    pub html_429: bool,
    /// Start and resend calls per network (client address) an hour ([`START_PER_NETWORK_HOURLY`]); on loopback every
    /// caller is one network, so a test sets it low.
    pub start_per_network: usize,
    /// Verify calls per network an hour ([`VERIFY_PER_NETWORK_HOURLY`]).
    pub verify_per_network: usize,
    /// `/auth/email/verify` answers 503 `busy` (the server's waiting places are full), with `Retry-After`.
    pub busy_verify: bool,
}

impl Default for MockOptions {
    fn default() -> Self {
        Self {
            auto_approve: false,
            code: Some(DEFAULT_CODE.into()),
            code_ttl: Duration::from_secs(300),
            resend_wait: Duration::from_secs(300),
            wrong_codes: false,
            expired_codes: false,
            no_email: false,
            banned: Vec::new(),
            html: false,
            html_429: false,
            start_per_network: START_PER_NETWORK_HOURLY,
            verify_per_network: VERIFY_PER_NETWORK_HOURLY,
            busy_verify: false,
        }
    }
}

#[derive(Clone)]
struct User {
    id: String,
    email: String,
}
struct Device {
    install: Option<String>,
    code: String,
    expires: Instant,
    decision: Option<Option<User>>,
    consumed: bool,
}
struct Grant {
    user: User,
    /// The sign-in this grant belongs to; a refresh keeps it.
    session: String,
    /// The install the sign-in was asked from (`install_id` at its start), if it said one.
    install: Option<String>,
    access: String,
    access_expires: Instant,
    refresh: String,
    refresh_expires: Instant,
}
/// One email sign-in request: its secret's hash, its current code and link, and when they stop working.
struct EmailRequest {
    email: String,
    secret_hash: String,
    code: String,
    link: String,
    started: Instant,
    expires: Instant,
    resend_at: Instant,
    attempts_left: u32,
    spent: bool,
    install: Option<String>,
}
/// A registered device: one row per account and install.
struct DeviceRow {
    id: String,
    /// The sign-in that registered it.
    session: String,
    platform: String,
    name: String,
}
#[derive(Default)]
struct State {
    options: MockOptions,
    devices: HashMap<String, Device>,
    users: HashMap<String, User>,
    sessions: HashMap<String, User>,
    grants: Vec<Grant>,
    email_requests: HashMap<String, EmailRequest>,
    /// Codes handed out so far (the counter behind [`MockOptions::code`]).
    codes_issued: u32,
    /// When a code was mailed, per address (the per-address limit, and what a test counts).
    mailed: HashMap<String, Vec<Instant>>,
    /// Start and resend calls, and verify calls, per network (the per-network limits).
    network_starts: HashMap<IpAddr, Vec<Instant>>,
    network_verifies: HashMap<IpAddr, Vec<Instant>>,
    /// By (account, install id); a registration without an install id gets a row of its own.
    registered: HashMap<(String, String), DeviceRow>,
    /// The sign-ins (grant sessions) that have registered their device: one each.
    registered_sessions: std::collections::HashSet<String>,
}

/// A running account server. Dropping it closes the listener.
pub struct MockAccount {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    task: tokio::task::JoinHandle<()>,
}
impl MockAccount {
    pub async fn start(addr: SocketAddr, auto_approve: bool) -> std::io::Result<Self> {
        Self::start_with(
            addr,
            MockOptions {
                auto_approve,
                ..MockOptions::default()
            },
        )
        .await
    }

    pub async fn start_with(addr: SocketAddr, options: MockOptions) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        let state = Arc::new(Mutex::new(State {
            options,
            ..State::default()
        }));
        let shared = state.clone();
        let task = tokio::spawn(async move {
            while let Ok((mut stream, peer)) = listener.accept().await {
                let state = shared.clone();
                tokio::spawn(async move {
                    let work = async {
                        let response = match read_request(&mut stream).await {
                            Ok(mut request) => {
                                request.peer = peer.ip();
                                println!(
                                    "[mock-account] {} {}",
                                    request.method,
                                    request.target.split('?').next().unwrap_or("/")
                                );
                                dispatch(
                                    &request,
                                    &mut state.lock().unwrap_or_else(|e| e.into_inner()),
                                )
                            }
                            Err(error) => oauth(400, "invalid_request", &error),
                        };
                        stream.write_all(response.wire().as_bytes()).await?;
                        stream.shutdown().await
                    };
                    let _ = tokio::time::timeout(Duration::from_secs(20), work).await;
                });
            }
        });
        Ok(Self { addr, state, task })
    }
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
    /// The code of the newest live email request for `email` (what the mail would say).
    pub fn code_for(&self, email: &str) -> Option<String> {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        newest_request(&state, email).map(|(_, request)| request.code.clone())
    }
    /// How many codes were mailed to `email` (starts and resends), as the mock prints them.
    pub fn codes_mailed(&self, email: &str) -> usize {
        let state = self.state.lock().unwrap();
        state
            .mailed
            .get(&email.to_ascii_lowercase())
            .map_or(0, Vec::len)
    }
    /// The link token of the newest live email request for `email`.
    pub fn link_for(&self, email: &str) -> Option<String> {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        newest_request(&state, email).map(|(_, request)| request.link.clone())
    }
    /// How many devices are registered (one row per account and install).
    pub fn registered_devices(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .registered
            .len()
    }
    /// The id, platform and name a device registered with, by install id.
    pub fn registered(&self, install_id: &str) -> Option<(String, String, String)> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .registered
            .iter()
            .find(|((_, install), _)| install == install_id)
            .map(|(_, row)| (row.id.clone(), row.platform.clone(), row.name.clone()))
    }
}
impl Drop for MockAccount {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn newest_request<'a>(state: &'a State, email: &str) -> Option<(&'a String, &'a EmailRequest)> {
    state
        .email_requests
        .iter()
        .filter(|(_, request)| request.email == email && !request.spent)
        .max_by_key(|(_, request)| request.expires)
}

struct Request {
    /// The caller's address: its network, for the per-network limits.
    peer: IpAddr,
    method: String,
    target: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}
async fn read_request(stream: &mut TcpStream) -> Result<Request, String> {
    let mut bytes = Vec::new();
    let end = loop {
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break end;
        }
        if bytes.len() > 16 * 1024 {
            return Err("request headers too large".into());
        }
        let mut buf = [0; 1024];
        let n = stream.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("incomplete request".into());
        }
        bytes.extend_from_slice(&buf[..n]);
    };
    if end > 16 * 1024 {
        return Err("request headers too large".into());
    }
    let head = std::str::from_utf8(&bytes[..end]).map_err(|_| "invalid headers")?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split_whitespace();
    let method = first.next().unwrap_or_default().to_owned();
    let target = first.next().unwrap_or_default().to_owned();
    let headers: HashMap<String, String> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    if headers.contains_key("transfer-encoding") {
        return Err("use content-length, not transfer-encoding".into());
    }
    let len = headers
        .get("content-length")
        .map(|s| s.parse::<usize>())
        .transpose()
        .map_err(|_| "invalid content-length")?
        .unwrap_or(0);
    if len > 16 * 1024 {
        return Err("request body too large".into());
    }
    let mut body = bytes[end + 4..].to_vec();
    while body.len() < len {
        let mut buf = vec![0; len - body.len()];
        let n = stream.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("incomplete body".into());
        }
        body.extend_from_slice(&buf[..n]);
    }
    body.truncate(len);
    if body.is_empty()
        && let Some(encoded) = headers.get("x-buwiz-request-body")
    {
        body = decode_b64(encoded).ok_or("invalid base64 body header")?;
    }
    Ok(Request {
        peer: IpAddr::from([127, 0, 0, 1]),
        method,
        target,
        headers,
        body,
    })
}

fn decode_b64(text: &str) -> Option<Vec<u8>> {
    data_encoding::BASE64.decode(text.as_bytes()).ok()
}

struct Response {
    status: u16,
    kind: &'static str,
    body: String,
    headers: Vec<(&'static str, String)>,
}
impl Response {
    fn wire(&self) -> String {
        let reason = match self.status {
            200 => "OK",
            303 => "See Other",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            409 => "Conflict",
            429 => "Too Many Requests",
            _ => "Error",
        };
        let extra: String = self
            .headers
            .iter()
            .map(|(k, v)| format!("{k}: {v}\r\n"))
            .collect();
        format!(
            "HTTP/1.1 {} {reason}\r\ncontent-type: {}\r\ncontent-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n{extra}\r\n{}",
            self.status,
            self.kind,
            self.body.len(),
            self.body
        )
    }
}
fn json_response(status: u16, body: Value) -> Response {
    Response {
        status,
        kind: "application/json",
        body: body.to_string(),
        headers: vec![],
    }
}
fn oauth(status: u16, error: &str, description: &str) -> Response {
    json_response(
        status,
        json!({"error":error,"error_description":description}),
    )
}
fn html(body: String) -> Response {
    Response {
        status: 200,
        kind: "text/html; charset=utf-8",
        body: format!(
            "<!doctype html><html><head><meta name=viewport content='width=device-width,initial-scale=1'><title>Buwiz mock account</title><style>body{{font:18px system-ui;max-width:520px;margin:60px auto;padding:24px;background:#f5f7fa;color:#182433}}main{{padding:28px;background:white;border-radius:16px}}input,button{{font:inherit;padding:12px;margin:8px 0;max-width:100%;box-sizing:border-box}}button{{cursor:pointer}}label,input{{display:block}}small{{color:#596579}}</style></head><body><main><small>BUWIZ · LOCAL DEVELOPMENT ONLY</small>{body}</main></body></html>"
        ),
        headers: vec![],
    }
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn form(text: &str) -> HashMap<String, String> {
    text.split('&')
        .filter_map(|s| s.split_once('='))
        .filter_map(|(k, v)| {
            Some((
                urlencoding::decode(&k.replace('+', " ")).ok()?.into_owned(),
                urlencoding::decode(&v.replace('+', " ")).ok()?.into_owned(),
            ))
        })
        .collect()
}
fn safe_next(s: &str) -> &str {
    let local = |path: &str| {
        path.starts_with('/')
            && !path.starts_with("//")
            && !path.chars().any(|c| c.is_control() || c == '\\')
    };
    if local(s) && urlencoding::decode(s).is_ok_and(|decoded| local(&decoded)) {
        s
    } else {
        "/device"
    }
}
fn redirect(next: &str) -> Response {
    Response {
        status: 303,
        kind: "text/plain",
        body: "Continue".into(),
        headers: vec![("location", next.to_owned())],
    }
}
fn token() -> String {
    Uuid::new_v4().to_string()
}
fn user(state: &mut State, email: &str) -> User {
    state
        .users
        .entry(email.to_owned())
        .or_insert_with(|| User {
            id: token(),
            email: email.to_owned(),
        })
        .clone()
}
fn issue(
    state: &mut State,
    user: User,
    session: Option<String>,
    install: Option<String>,
) -> Response {
    let access = token();
    let refresh = token();
    state.grants.push(Grant {
        user,
        session: session.unwrap_or_else(token),
        install,
        access: access.clone(),
        access_expires: Instant::now() + TTL,
        refresh: refresh.clone(),
        refresh_expires: Instant::now() + Duration::from_secs(90 * 86400),
    });
    json_response(
        200,
        json!({"access_token":access,"refresh_token":refresh,"token_type":"Bearer","expires_in":900,"scope":"app"}),
    )
}

fn dispatch(r: &Request, state: &mut State) -> Response {
    let auto = state.options.auto_approve;
    let (path, query) = r.target.split_once('?').unwrap_or((&r.target, ""));
    let query = form(query);
    let body = String::from_utf8_lossy(&r.body);
    let fields = form(&body);
    let value: Value = serde_json::from_slice(&r.body).unwrap_or(Value::Null);
    let get = |k: &str| value.get(k).and_then(Value::as_str).unwrap_or_default();
    let field = |k: &str| fields.get(k).map(String::as_str).unwrap_or_default();
    let session = r
        .headers
        .get("cookie")
        .and_then(|c| {
            c.split(';')
                .find_map(|s| s.trim().strip_prefix("mock_session="))
        })
        .and_then(|s| state.sessions.get(s))
        .cloned();
    match (r.method.as_str(), path) {
        ("POST", "/auth/device/start") => {
            if get("client_id") != "buwiz-desktop" {
                return oauth(401, "invalid_client", "unknown client_id");
            }
            let install = value
                .get("install_id")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if install.as_deref().is_some_and(|id| !valid_install_id(id)) {
                return oauth(400, "bad_install_id", "malformed install_id");
            }
            let device = token();
            let code = token()[..8].to_ascii_uppercase();
            let code = format!("{}-{}", &code[..4], &code[4..]);
            let decision = if auto {
                Some(Some(user(state, DEFAULT_EMAIL)))
            } else {
                None
            };
            state.devices.insert(
                device.clone(),
                Device {
                    install,
                    code: code.clone(),
                    expires: Instant::now() + TTL,
                    decision,
                    consumed: false,
                },
            );
            println!("[mock-account] user code: {code}");
            let host = r
                .headers
                .get("host")
                .map(String::as_str)
                .unwrap_or("localhost:3008");
            let uri = format!("http://{host}/device");
            json_response(
                200,
                json!({"device_code":device,"user_code":code,"verification_uri":uri,"verification_uri_complete":format!("{uri}?user_code={code}"),"expires_in":900,"interval":5}),
            )
        }
        ("POST", "/auth/device/poll") => {
            if get("client_id") != "buwiz-desktop" {
                return oauth(401, "invalid_client", "unknown client_id");
            }
            let Some(d) = state.devices.get_mut(get("device_code")) else {
                return oauth(400, "invalid_grant", "unknown device code");
            };
            if d.consumed {
                return oauth(400, "invalid_grant", "device code already used");
            }
            if d.expires <= Instant::now() {
                return oauth(400, "expired_token", "sign-in request expired");
            }
            match d.decision.clone() {
                None => oauth(400, "authorization_pending", "waiting for approval"),
                Some(None) => oauth(400, "access_denied", "sign-in declined"),
                Some(Some(user)) => {
                    d.consumed = true;
                    let install = d.install.clone();
                    issue(state, user, None, install)
                }
            }
        }
        ("GET", "/me") | ("POST", "/api/auth/logout") => {
            let bearer = r
                .headers
                .get("authorization")
                .and_then(|s| s.strip_prefix("Bearer "))
                .unwrap_or_default();
            let Some(i) = state
                .grants
                .iter()
                .position(|g| g.access == bearer && g.access_expires > Instant::now())
            else {
                return oauth(401, "invalid_token", "invalid access token");
            };
            if path == "/me" {
                let u = &state.grants[i].user;
                json_response(200, json!({"user_id":u.id,"email":u.email}))
            } else {
                state.grants.remove(i);
                json_response(200, json!({}))
            }
        }
        ("POST", "/oauth/token") => {
            if field("client_id") != "buwiz-desktop" {
                return oauth(401, "invalid_client", "unknown client_id");
            }
            if field("grant_type") != "refresh_token" {
                return oauth(400, "unsupported_grant_type", "use refresh_token");
            }
            let Some(i) = state.grants.iter().position(|g| {
                g.refresh == field("refresh_token") && g.refresh_expires > Instant::now()
            }) else {
                return oauth(400, "invalid_grant", "invalid refresh token");
            };
            let old = state.grants.remove(i);
            issue(state, old.user, Some(old.session), old.install)
        }
        ("GET", "/login" | "/signup") => {
            let next = safe_next(query.get("next").map(String::as_str).unwrap_or("/device"));
            html(format!(
                "<h1>{}</h1><p>Use a development email. No password or email code is required.</p><form method=post action=/login><label>Email<input type=email name=email value='{DEFAULT_EMAIL}' required></label><input type=hidden name=next value='{}'><button>Continue</button></form>",
                if path == "/signup" {
                    "Create account"
                } else {
                    "Sign in"
                },
                escape(next)
            ))
        }
        ("POST", "/login" | "/signup") => {
            let email = field("email").trim();
            if email.is_empty() || !email.contains('@') || email.len() > 254 {
                return oauth(400, "invalid_request", "enter a development email");
            }
            let u = user(state, email);
            let id = token();
            state.sessions.insert(id.clone(), u);
            let mut response = redirect(safe_next(field("next")));
            response.headers.push((
                "set-cookie",
                format!("mock_session={id}; Path=/; HttpOnly; SameSite=Lax"),
            ));
            response
        }
        ("GET", "/device") => {
            let code = query
                .get("user_code")
                .map(String::as_str)
                .unwrap_or_default();
            if session.is_none() {
                return redirect(&format!("/login?next={}", urlencoding::encode(&r.target)));
            }
            if !state
                .devices
                .values()
                .any(|d| d.code == code && d.expires > Instant::now() && !d.consumed)
            {
                return html(
                    "<h1>Unknown or expired device code</h1><p>Start sign-in again in the app.</p>"
                        .into(),
                );
            }
            html(format!(
                "<h1>Approve this device?</h1><p>Signed in as <strong>{}</strong></p><h2>{}</h2><form method=post action=/device/decision><input type=hidden name=user_code value='{}'><button name=decision value=approve>Approve</button> <button name=decision value=deny>Deny</button></form>",
                escape(&session.unwrap().email),
                escape(code),
                escape(code)
            ))
        }
        ("POST", "/device/decision") => {
            let Some(u) = session else {
                return oauth(401, "invalid_request", "sign in first");
            };
            if !matches!(field("decision"), "approve" | "deny") {
                return oauth(400, "invalid_request", "choose approve or deny");
            }
            let Some(d) = state.devices.values_mut().find(|d| {
                d.code == field("user_code") && !d.consumed && d.expires > Instant::now()
            }) else {
                return oauth(400, "expired_token", "unknown or expired device code");
            };
            d.decision = Some(if field("decision") == "approve" {
                Some(u)
            } else {
                None
            });
            html(format!(
                "<h1>Device {}</h1><p>You can return to the Buwiz app.</p>",
                if field("decision") == "approve" {
                    "approved"
                } else {
                    "denied"
                }
            ))
        }
        ("POST", "/auth/email/start" | "/auth/email/resend" | "/auth/email/verify")
            if state.options.no_email =>
        {
            oauth(404, "not_found", "no such endpoint")
        }
        ("POST", "/auth/email/start" | "/auth/email/resend" | "/auth/email/verify")
            if state.options.html || state.options.html_429 =>
        {
            let mut page = html("<h1>Just a moment...</h1><p>Checking your browser.</p>".into());
            if state.options.html_429 {
                page.status = 429;
            }
            page
        }
        ("POST", "/auth/email/start" | "/auth/email/resend") => {
            let limit = state.options.start_per_network;
            if let Some(refused) = per_network(&mut state.network_starts, r.peer, limit) {
                return refused;
            }
            if path == "/auth/email/start" {
                email_start(state, &value)
            } else {
                email_resend(state, &value)
            }
        }
        ("POST", "/auth/email/verify") if state.options.busy_verify => {
            let mut busy = failure(
                503,
                "busy",
                "The server is busy. Try again in a moment.",
                json!({}),
            );
            busy.headers.push(("retry-after", "5".into()));
            busy
        }
        ("POST", "/auth/email/verify") => {
            let limit = state.options.verify_per_network;
            if let Some(refused) = per_network(&mut state.network_verifies, r.peer, limit) {
                return refused;
            }
            email_verify(state, &value)
        }
        ("POST", "/api/devices") => register_device(r, state, &value),
        _ => oauth(404, "not_found", "no such endpoint"),
    }
}

/// The contract's error: `{"error": code, "message": sentence}` and whatever else the code carries; a `retry_after` is
/// also the `Retry-After` header.
fn failure(status: u16, code: &str, message: &str, extra: Value) -> Response {
    let mut body = json!({"error": code, "message": message});
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            body.insert(key.clone(), value.clone());
        }
    }
    let retry = body.get("retry_after").and_then(Value::as_u64);
    let mut response = json_response(status, body);
    if let Some(seconds) = retry {
        response.headers.push(("retry-after", seconds.to_string()));
    }
    response
}

/// The per-address limit: `too_many_requests` once an hour holds [`PER_ADDRESS_HOURLY`] codes for the address. As the
/// server's, its `retry_after` is a fixed hour, not the time until the oldest code is an hour old.
fn over_the_limit(state: &mut State, email: &str) -> Option<Response> {
    let hour = Duration::from_secs(3600);
    let mailed = state.mailed.entry(email.to_owned()).or_default();
    mailed.retain(|at| at.elapsed() < hour);
    if mailed.len() < PER_ADDRESS_HOURLY {
        return None;
    }
    Some(failure(
        429,
        "too_many_requests",
        "Too many sign-in codes were sent to this address. Try again later.",
        json!({"retry_after": hour.as_secs()}),
    ))
}

/// A per-network limit: `limit` calls an hour from `peer`, every call counted, answered or not; the next is
/// `too_many_requests` with an hour's wait (as the per-address limit's), and is not counted.
fn per_network(
    calls: &mut HashMap<IpAddr, Vec<Instant>>,
    peer: IpAddr,
    limit: usize,
) -> Option<Response> {
    let hour = Duration::from_secs(3600);
    let made = calls.entry(peer).or_default();
    made.retain(|at| at.elapsed() < hour);
    if made.len() >= limit {
        return Some(failure(
            429,
            "too_many_requests",
            "Too many requests from this network. Try again later.",
            json!({"retry_after": hour.as_secs()}),
        ));
    }
    made.push(Instant::now());
    None
}

/// A code went out to `email` (counted for the limit and for tests).
fn mailed(state: &mut State, email: &str) {
    state
        .mailed
        .entry(email.to_owned())
        .or_default()
        .push(Instant::now());
}

fn secret_hash(secret: &str) -> String {
    data_encoding::BASE64URL_NOPAD.encode(&Sha256::digest(secret.as_bytes()))
}

fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && email.len() <= 254
        && !email.chars().any(char::is_whitespace)
        && !domain.contains('@')
}

fn valid_install_id(id: &str) -> bool {
    (16..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn seconds_until(when: Instant) -> u64 {
    when.saturating_duration_since(Instant::now())
        .as_secs_f64()
        .ceil() as u64
}

/// The next code: the first one counted up by the codes handed out so far (or random), so each differs from the last.
fn new_code(state: &mut State) -> String {
    let issued = state.codes_issued;
    state.codes_issued += 1;
    let n = match state.options.code.as_deref().map(str::parse::<u32>) {
        Some(Ok(first)) => first.wrapping_add(issued),
        _ => u32::from_le_bytes(rand::random::<[u8; 4]>()),
    } % 1_000_000;
    format!("{n:06}")
}

/// A request's code and link start over: a new pair (the old link is now `link_used`), a new life, Resend locked again.
fn reissue(options: &MockOptions, request: &mut EmailRequest, code: String) {
    request.link = token();
    request.code = code;
    request.expires = Instant::now() + options.code_ttl;
    request.resend_at = Instant::now() + options.resend_wait;
    request.attempts_left = ATTEMPTS;
}

fn email_answer(options: &MockOptions, request_id: &str, request: &EmailRequest) -> Response {
    println!(
        "[mock-account] sign-in code for {}: {}",
        request.email, request.code
    );
    json_response(
        200,
        json!({
            "request_id": request_id,
            "expires_in": options.code_ttl.as_secs(),
            "resend_after": options.resend_wait.as_secs(),
            "code_length": 6,
        }),
    )
}

fn email_start(state: &mut State, value: &Value) -> Response {
    let get = |k: &str| value.get(k).and_then(Value::as_str).unwrap_or_default();
    if !APP_CLIENTS.contains(&get("client_id")) {
        return failure(
            400,
            "unknown_client",
            "This app is not known to the server.",
            json!({}),
        );
    }
    let email = get("email").trim().to_ascii_lowercase();
    if !valid_email(&email) {
        return failure(
            400,
            "invalid_email",
            "That email address does not look right.",
            json!({}),
        );
    }
    let install = value
        .get("install_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if install.as_deref().is_some_and(|id| !valid_install_id(id)) {
        return failure(
            400,
            "bad_install_id",
            "The install id is malformed.",
            json!({}),
        );
    }
    // `intent` only picks the mail's words and an unknown `platform` is stored as nothing: neither is an error.
    if get("secret_hash").len() != 43 {
        return failure(
            400,
            "invalid_request",
            "The sign-in request is incomplete.",
            json!({}),
        );
    }
    // One wait: the same address from the same install while its code still works.
    let held = install.as_deref().and_then(|install| {
        state
            .email_requests
            .values()
            // Only while a usable code exists: one whose tries are used up is no reason to wait.
            .filter(|r| {
                r.email == email && !r.spent && r.expires > Instant::now() && r.attempts_left > 0
            })
            .find(|r| r.install.as_deref() == Some(install))
    });
    if let Some(live) = held {
        let wait = seconds_until(live.expires);
        return failure(
            429,
            "code_already_sent",
            &format!(
                "A sign-in code was already sent to this address. Use it, or try again in {} minutes.",
                wait.div_ceil(60).max(1)
            ),
            json!({"retry_after": wait}),
        );
    }
    if let Some(refused) = over_the_limit(state, &email) {
        return refused;
    }
    let options = state.options.clone();
    let request_id = token();
    let request = EmailRequest {
        email,
        secret_hash: get("secret_hash").to_owned(),
        code: new_code(state),
        link: token(),
        started: Instant::now(),
        expires: Instant::now() + options.code_ttl,
        resend_at: Instant::now() + options.resend_wait,
        attempts_left: ATTEMPTS,
        spent: false,
        install,
    };
    let answer = email_answer(&options, &request_id, &request);
    mailed(state, &request.email);
    state.email_requests.insert(request_id, request);
    answer
}

/// The request named by `request_id`, if this caller holds its secret and it is not spent.
fn own_request<'a>(
    state: &'a mut State,
    value: &Value,
) -> Result<(String, &'a mut EmailRequest), Response> {
    let get = |k: &str| value.get(k).and_then(Value::as_str).unwrap_or_default();
    let id = get("request_id").to_owned();
    let hash = secret_hash(get("secret"));
    match state.email_requests.get_mut(&id) {
        Some(request) if request.secret_hash == hash && !request.spent => Ok((id, request)),
        _ => Err(failure(
            400,
            "invalid_request",
            "This sign-in request is no longer valid. Start again.",
            json!({}),
        )),
    }
}

fn email_resend(state: &mut State, value: &Value) -> Response {
    let options = state.options.clone();
    let (id, request) = match own_request(state, value) {
        Ok(found) => found,
        Err(response) => return response,
    };
    if request.started.elapsed() > Duration::from_secs(3600) {
        return failure(
            400,
            "invalid_request",
            "This sign-in request has ended. Start again.",
            json!({}),
        );
    }
    if request.resend_at > Instant::now() {
        let wait = seconds_until(request.resend_at);
        return failure(
            429,
            "too_many_requests",
            "A new code can be sent once this one has expired.",
            json!({"retry_after": wait}),
        );
    }
    let email = request.email.clone();
    if let Some(refused) = over_the_limit(state, &email) {
        return refused;
    }
    let code = new_code(state);
    let Some(request) = state.email_requests.get_mut(&id) else {
        return failure(
            400,
            "invalid_request",
            "This sign-in request is no longer valid. Start again.",
            json!({}),
        );
    };
    reissue(&options, request, code);
    let answer = email_answer(&options, &id, request);
    mailed(state, &email);
    answer
}

/// `/auth/email/verify`, its checks in the server's order: (1) the request exists and is unused and (2) the secret is
/// its own, else `invalid_request`; (3) it has not expired, else `code_expired` (whatever was sent); (4) the code (no
/// tries left: `too_many_attempts`; wrong: `invalid_code` with the tries left, or `too_many_attempts` on the last) or the
/// link (wrong or replaced by a Resend: `link_used`); a right one is taken, the request spent; (5) then a banned account
/// is `account_banned` (its request stays spent); (6) the tokens.
fn email_verify(state: &mut State, value: &Value) -> Response {
    let options = state.options.clone();
    let get = |k: &str| value.get(k).and_then(Value::as_str).unwrap_or_default();
    let (code, link) = (get("code").to_owned(), get("link_token").to_owned());
    let (_, request) = match own_request(state, value) {
        Ok(found) => found,
        Err(response) => return response,
    };
    if options.expired_codes || request.expires <= Instant::now() {
        return failure(
            400,
            "code_expired",
            "This code has expired. Get a new one.",
            json!({}),
        );
    }
    if link.is_empty() {
        let too_many = || {
            failure(
                400,
                "too_many_attempts",
                "Too many wrong codes. Send a new code to try again.",
                json!({}),
            )
        };
        if request.attempts_left == 0 {
            return too_many();
        }
        if options.wrong_codes || code != request.code {
            request.attempts_left -= 1;
            if request.attempts_left == 0 {
                return too_many();
            }
            return failure(
                400,
                "invalid_code",
                "That code is not right.",
                json!({"attempts_left": request.attempts_left}),
            );
        }
    } else if link != request.link {
        return failure(
            400,
            "link_used",
            "This sign-in link was already used.",
            json!({}),
        );
    }
    // The right code or link is taken and the request spent; only then is a ban checked.
    request.spent = true;
    let email = request.email.clone();
    let install = request.install.clone();
    if options
        .banned
        .iter()
        .any(|banned| banned.eq_ignore_ascii_case(&email))
    {
        return failure(403, "account_banned", "This account is closed.", json!({}));
    }
    let u = user(state, &email);
    issue(state, u, None, install)
}

/// `POST /api/devices` with the bearer token. One registration per sign-in (`409 device_registered`); with an install
/// id one row per account and install, updated in place (same id) by a later sign-in; `install_mismatch` when the
/// sign-in named another install.
fn register_device(r: &Request, state: &mut State, value: &Value) -> Response {
    let bearer = r
        .headers
        .get("authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or_default();
    let Some(grant) = state
        .grants
        .iter()
        .find(|g| g.access == bearer && g.access_expires > Instant::now())
    else {
        return oauth(401, "invalid_token", "invalid access token");
    };
    let (session, user_id, granted) = (
        grant.session.clone(),
        grant.user.id.clone(),
        grant.install.clone(),
    );
    let get = |k: &str| value.get(k).and_then(Value::as_str).unwrap_or_default();
    let install = value
        .get("install_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if get("public_key").is_empty() {
        return failure(
            400,
            "invalid_request",
            "The device registration is incomplete.",
            json!({}),
        );
    }
    if install.as_deref().is_some_and(|id| !valid_install_id(id)) {
        return failure(
            400,
            "bad_install_id",
            "The install id is malformed.",
            json!({}),
        );
    }
    if granted.is_some() && granted != install {
        return failure(
            400,
            "install_mismatch",
            "This sign-in was made on another install.",
            json!({}),
        );
    }
    // A sign-in without an install id may not take an install whose device belongs to another live sign-in.
    if granted.is_none()
        && let Some(install) = &install
        && let Some(row) = state.registered.get(&(user_id.clone(), install.clone()))
        && row.session != session
        && state
            .grants
            .iter()
            .any(|g| g.session == row.session && g.refresh_expires > Instant::now())
    {
        return failure(
            409,
            "install_in_use",
            "This device belongs to another sign-in.",
            json!({}),
        );
    }
    if state.registered_sessions.contains(&session) {
        return failure(
            409,
            "device_registered",
            "This sign-in has already added its device.",
            json!({}),
        );
    }
    state.registered_sessions.insert(session.clone());
    let platform = Some(get("platform"))
        .filter(|p| PLATFORMS.contains(p))
        .unwrap_or_default()
        .to_owned();
    let key = (user_id, install.unwrap_or_else(token));
    let id = state
        .registered
        .get(&key)
        .map(|row| row.id.clone())
        .unwrap_or_else(token);
    state.registered.insert(
        key,
        DeviceRow {
            id: id.clone(),
            session: session.clone(),
            platform,
            name: get("name").to_owned(),
        },
    );
    json_response(200, json!({"id": id}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_account::{self as cloud, BlockingHttp, CloudError, Http, HttpRequest};

    fn browser_decision(url: &str, code: &str, decision: &str) {
        let browser = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let page = browser
            .get(format!("{url}/signup?next=%2Fdevice"))
            .send()
            .unwrap()
            .text()
            .unwrap();
        assert!(page.contains(DEFAULT_EMAIL));
        let response = browser
            .post(format!("{url}/login"))
            .form(&[("email", "tester@buwiz.local"), ("next", "/device")])
            .send()
            .unwrap();
        assert_eq!(response.status(), 303);
        let cookie = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let response = browser
            .get(format!("{url}/device?user_code={code}"))
            .header("cookie", &cookie)
            .send()
            .unwrap();
        assert!(response.text().unwrap().contains("Approve this device?"));
        let response = browser
            .post(format!("{url}/device/decision"))
            .header("cookie", &cookie)
            .form(&[("user_code", code), ("decision", decision)])
            .send()
            .unwrap();
        assert_eq!(response.status(), 200);
    }

    fn request(
        http: &mut BlockingHttp,
        url: &str,
        path: &str,
        body: String,
    ) -> cloud::HttpResponse {
        http.exchange(&HttpRequest {
            method: "POST",
            url: format!("{url}{path}"),
            content_type: "application/json",
            bearer: None,
            body,
        })
        .unwrap()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn real_client_browser_approval_refresh_and_logout() {
        let server = MockAccount::start("127.0.0.1:0".parse().unwrap(), false)
            .await
            .unwrap();
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            let challenge =
                cloud::start_device(&mut http, &url, cloud::unix_now(), INSTALL).unwrap();
            browser_decision(&url, &challenge.user_code, "approve");
            let tokens =
                cloud::poll_until(&mut http, &challenge, &mut cloud::unix_now, &mut |_| {
                    panic!("already approved")
                })
                .unwrap();
            let (id, email, _) =
                cloud::load_profile(&mut http, &url, &tokens.access_token).unwrap();
            assert_eq!(email.as_deref(), Some("tester@buwiz.local"));
            let next = cloud::refresh_tokens(&mut http, &url, &tokens.refresh_token).unwrap();
            assert_ne!(tokens.refresh_token, next.refresh_token);
            assert!(matches!(
                cloud::refresh_tokens(&mut http, &url, &tokens.refresh_token),
                Err(CloudError::InvalidGrant)
            ));
            assert_eq!(
                cloud::load_profile(&mut http, &url, &next.access_token)
                    .unwrap()
                    .0,
                id
            );
            cloud::revoke_session(
                &mut http,
                &cloud::AccountSecrets {
                    server_url: url.clone(),
                    access_token: next.access_token.clone(),
                    refresh_token: next.refresh_token.clone(),
                    access_expires_at_unix: 0,
                    user_id: id,
                    email,
                    offline: false,
                    practice: false,
                },
            );
            assert!(matches!(
                cloud::load_profile(&mut http, &url, &next.access_token),
                Err(CloudError::InvalidGrant)
            ));
            assert!(matches!(
                cloud::refresh_tokens(&mut http, &url, &next.refresh_token),
                Err(CloudError::InvalidGrant)
            ));
        })
        .await
        .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pending_denial_empty_and_header_fallback() {
        let server = MockAccount::start("127.0.0.1:0".parse().unwrap(), false)
            .await
            .unwrap();
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            assert_eq!(
                request(&mut http, &url, "/auth/device/start", "{}".into()).status,
                401
            );
            let browser = reqwest::blocking::Client::new();
            let response = browser
                .post(format!("{url}/auth/device/start"))
                .send()
                .unwrap();
            assert_eq!(response.status(), 401);
            let response = browser
                .post(format!("{url}/auth/device/start"))
                .header(
                    "x-buwiz-request-body",
                    "eyJjbGllbnRfaWQiOiJidXdpei1kZXNrdG9wIn0=",
                )
                .send()
                .unwrap();
            assert_eq!(response.status(), 200);
            let response = browser
                .post(format!("{url}/auth/device/start"))
                .header("x-buwiz-request-body", "not-base64!")
                .body(r#"{"client_id":"buwiz-desktop"}"#)
                .send()
                .unwrap();
            assert_eq!(response.status(), 200);
            let response = browser
                .post(format!("{url}/auth/device/start"))
                .header("x-buwiz-request-body", "!")
                .send()
                .unwrap();
            assert_eq!(response.status(), 400);
            let challenge =
                cloud::start_device(&mut http, &url, cloud::unix_now(), INSTALL).unwrap();
            let body = json!({"client_id":"buwiz-desktop","device_code":challenge.device_code})
                .to_string();
            let pending = request(&mut http, &url, "/auth/device/poll", body.clone());
            assert_eq!(pending.status, 400);
            assert_eq!(
                serde_json::from_str::<Value>(&pending.body).unwrap()["error"],
                "authorization_pending"
            );
            browser_decision(&url, &challenge.user_code, "deny");
            let denied = request(&mut http, &url, "/auth/device/poll", body);
            assert_eq!(
                serde_json::from_str::<Value>(&denied.body).unwrap()["error"],
                "access_denied"
            );
            assert!(
                cloud::poll_until(
                    &mut http,
                    &challenge,
                    &mut cloud::unix_now,
                    &mut |_| panic!("denied")
                )
                .is_err()
            );
        })
        .await
        .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn auto_approve() {
        let server = MockAccount::start("127.0.0.1:0".parse().unwrap(), true)
            .await
            .unwrap();
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            let challenge =
                cloud::start_device(&mut http, &url, cloud::unix_now(), INSTALL).unwrap();
            let tokens =
                cloud::poll_until(&mut http, &challenge, &mut cloud::unix_now, &mut |_| {
                    panic!("auto approved")
                })
                .unwrap();
            assert_eq!(
                cloud::load_profile(&mut http, &url, &tokens.access_token)
                    .unwrap()
                    .1
                    .as_deref(),
                Some(DEFAULT_EMAIL)
            );
            let used = request(
                &mut http,
                &url,
                "/auth/device/poll",
                json!({"client_id":"buwiz-desktop","device_code":challenge.device_code})
                    .to_string(),
            );
            assert_eq!(
                serde_json::from_str::<Value>(&used.body).unwrap()["error"],
                "invalid_grant"
            );
        })
        .await
        .unwrap();
    }

    #[test]
    fn return_paths_stay_local() {
        for path in [
            "",
            "https://evil.com",
            "//evil.com",
            "/\\evil.com",
            "/%2F/evil",
            "/%5cevil.com",
            "/\t/evil.com",
            "/\n/evil.com",
            "/%09/evil.com",
            "/%0d%0aLocation:evil",
        ] {
            assert_eq!(safe_next(path), "/device", "{path:?}");
        }
        for path in [
            "/",
            "/device",
            "/device?user_code=ABCD-EFGH",
            "/device?name=Jane%20Doe",
        ] {
            assert_eq!(safe_next(path), path);
        }
    }

    // ------------------------------------------------------------ the email sign-in

    const INSTALL: &str = "install-id-of-this-test-0001";

    fn post(http: &mut BlockingHttp, url: &str, path: &str, body: Value) -> (u16, Value) {
        let answer = request(http, url, path, body.to_string());
        (
            answer.status,
            serde_json::from_str(&answer.body).unwrap_or(Value::Null),
        )
    }

    fn start_body(email: &str, secret: &str) -> Value {
        json!({"client_id":"buwiz-desktop","email":email,"intent":"sign_in","secret_hash":secret_hash(secret),"platform":"macos","install_id":INSTALL})
    }

    async fn mock(options: MockOptions) -> MockAccount {
        MockAccount::start_with("127.0.0.1:0".parse().unwrap(), options)
            .await
            .unwrap()
    }

    #[test]
    fn the_secret_hash_is_unpadded_base64url_sha256() {
        assert_eq!(
            secret_hash("abc"),
            "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0"
        );
        assert!(valid_install_id("abcdefghijklmnop"));
        assert!(!valid_install_id("short"));
        assert!(!valid_install_id("has a space in it, sixteen+"));
        assert!(!valid_install_id(&"a".repeat(65)));
        assert!(valid_email("a@b.co"));
        for bad in ["", "a", "a@b", "@b.co", "a@b.", "a b@c.co", "a@b@c.co"] {
            assert!(!valid_email(bad), "{bad}");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_contract_s_clients_limit_and_retry_after() {
        let server = mock(MockOptions::default()).await;
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            // The contract's client id and this app's are both served.
            let mut body = start_body("limit@example.test", "s0");
            body["client_id"] = json!("buwiz-ebirforms");
            body.as_object_mut().unwrap().remove("install_id");
            let (status, _) = post(&mut http, &url, "/auth/email/start", body.clone());
            assert_eq!(status, 200);
            // No install id is never held; the per-address limit is 5 codes an hour.
            for _ in 1..PER_ADDRESS_HOURLY {
                assert_eq!(
                    post(&mut http, &url, "/auth/email/start", body.clone()).0,
                    200
                );
            }
            // The sixth: a fixed hour's wait, in the body and the Retry-After header alike.
            let sixth = reqwest::blocking::Client::new()
                .post(format!("{url}/auth/email/start"))
                .header("content-type", "application/json")
                .body(body.to_string())
                .send()
                .unwrap();
            assert_eq!(sixth.status(), 429);
            assert_eq!(sixth.headers()["retry-after"], "3600");
            let refused: Value = sixth.json().unwrap();
            assert_eq!(refused["error"], "too_many_requests");
            assert_eq!(refused["retry_after"], 3600);
            assert!(refused["message"].as_str().is_some_and(|m| !m.is_empty()));
            assert_eq!(
                server.codes_mailed("limit@example.test"),
                PER_ADDRESS_HOURLY
            );
            // A retry_after is also the Retry-After header.
            let raw = failure(
                429,
                "code_already_sent",
                "Wait.",
                json!({"retry_after": 42}),
            )
            .wire();
            assert!(raw.contains("retry-after: 42\r\n"), "{raw}");
        })
        .await
        .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_code_signs_in_once_and_a_wrong_one_says_the_tries_left() {
        let server = mock(MockOptions::default()).await;
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            let (status, started) = post(
                &mut http,
                &url,
                "/auth/email/start",
                start_body("Filer@Example.test", "s1"),
            );
            assert_eq!(status, 200, "{started}");
            assert_eq!(started["expires_in"], 300);
            assert_eq!(started["resend_after"], 300);
            assert_eq!(started["code_length"], 6);
            let id = started["request_id"].as_str().unwrap().to_owned();
            assert_eq!(
                server.code_for("filer@example.test").as_deref(),
                Some(DEFAULT_CODE)
            );
            let verify =
                |code: &str, secret: &str| json!({"request_id":id,"secret":secret,"code":code});
            let (status, wrong) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                verify("000000", "s1"),
            );
            assert_eq!(
                (status, wrong["error"].as_str()),
                (400, Some("invalid_code"))
            );
            assert_eq!(wrong["attempts_left"], 4);
            assert!(wrong["message"].as_str().is_some());
            // The right code with someone else's secret: not this caller's request.
            let (status, other) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                verify(DEFAULT_CODE, "s2"),
            );
            assert_eq!(
                (status, other["error"].as_str()),
                (400, Some("invalid_request"))
            );
            let (status, tokens) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                verify(DEFAULT_CODE, "s1"),
            );
            assert_eq!(status, 200, "{tokens}");
            let access = tokens["access_token"].as_str().unwrap();
            assert!(tokens["refresh_token"].as_str().is_some());
            let (_, email, _) = cloud::load_profile(&mut http, &url, access).unwrap();
            assert_eq!(email.as_deref(), Some("filer@example.test"));
            // Spent: the same code again is a request to start over.
            let (status, spent) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                verify(DEFAULT_CODE, "s1"),
            );
            assert_eq!(
                (status, spent["error"].as_str()),
                (400, Some("invalid_request"))
            );
        })
        .await
        .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_code_from_another_request_is_invalid_code() {
        let server = mock(MockOptions::default()).await;
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            let (_, a) = post(
                &mut http,
                &url,
                "/auth/email/start",
                start_body("a@example.test", "secret-a"),
            );
            let (_, b) = post(
                &mut http,
                &url,
                "/auth/email/start",
                start_body("b@example.test", "secret-b"),
            );
            let code_a = server.code_for("a@example.test").unwrap();
            let code_b = server.code_for("b@example.test").unwrap();
            // Each request has its own code: the first is the default, the next counts up.
            assert_eq!((code_a.as_str(), code_b.as_str()), (DEFAULT_CODE, "246811"));
            // Device B types the code of A's mail: B's request, B's secret, A's code.
            let (status, wrong) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                json!({"request_id":b["request_id"],"secret":"secret-b","code":code_a}),
            );
            assert_eq!(
                (status, wrong["error"].as_str()),
                (400, Some("invalid_code"))
            );
            // A's request without A's secret is no one's.
            let (status, stolen) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                json!({"request_id":a["request_id"],"secret":"secret-b","code":code_a}),
            );
            assert_eq!(
                (status, stolen["error"].as_str()),
                (400, Some("invalid_request"))
            );
            let (status, _) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                json!({"request_id":a["request_id"],"secret":"secret-a","code":code_a}),
            );
            assert_eq!(status, 200);
        })
        .await
        .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn resend_waits_for_the_expiry_and_the_old_code_and_link_stop() {
        let server = mock(MockOptions {
            code_ttl: Duration::from_secs(1),
            resend_wait: Duration::from_secs(1),
            ..MockOptions::default()
        })
        .await;
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            let (_, started) = post(
                &mut http,
                &url,
                "/auth/email/start",
                start_body("r@example.test", "s"),
            );
            assert_eq!(started["expires_in"], 1);
            let id = started["request_id"].clone();
            let (status, early) = post(
                &mut http,
                &url,
                "/auth/email/resend",
                json!({"request_id":id,"secret":"s"}),
            );
            assert_eq!(
                (status, early["error"].as_str()),
                (429, Some("too_many_requests"))
            );
            assert!(early["retry_after"].as_u64().unwrap() <= 1);
            // The same address again while its code lives: already sent.
            let (status, again) = post(
                &mut http,
                &url,
                "/auth/email/start",
                start_body("r@example.test", "t"),
            );
            assert_eq!(
                (status, again["error"].as_str()),
                (429, Some("code_already_sent"))
            );
            let old_code = server.code_for("r@example.test").unwrap();
            let old_link = server.link_for("r@example.test").unwrap();
            std::thread::sleep(Duration::from_millis(1100));
            let (status, expired) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                json!({"request_id":id,"secret":"s","code":old_code}),
            );
            assert_eq!(
                (status, expired["error"].as_str()),
                (400, Some("code_expired"))
            );
            let (status, resent) = post(
                &mut http,
                &url,
                "/auth/email/resend",
                json!({"request_id":id,"secret":"s"}),
            );
            assert_eq!(status, 200, "{resent}");
            assert_eq!(resent["request_id"], id);
            let (status, used) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                json!({"request_id":id,"secret":"s","link_token":old_link}),
            );
            assert_eq!((status, used["error"].as_str()), (400, Some("link_used")));
            let new_code = server.code_for("r@example.test").unwrap();
            assert_ne!(new_code, old_code, "a Resend's code is a new one");
            let (status, stale) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                json!({"request_id":id,"secret":"s","code":old_code}),
            );
            assert_eq!(
                (status, stale["error"].as_str()),
                (400, Some("invalid_code"))
            );
            let link = server.link_for("r@example.test").unwrap();
            let (status, _) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                json!({"request_id":id,"secret":"s","link_token":link}),
            );
            assert_eq!(status, 200);
        })
        .await
        .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn five_wrong_codes_are_too_many_and_bad_starts_say_why() {
        let server = mock(MockOptions::default()).await;
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            let (_, started) = post(
                &mut http,
                &url,
                "/auth/email/start",
                start_body("t@example.test", "s"),
            );
            let id = started["request_id"].clone();
            let mut lefts = Vec::new();
            for _ in 0..5 {
                let (_, answer) = post(
                    &mut http,
                    &url,
                    "/auth/email/verify",
                    json!({"request_id":id,"secret":"s","code":"111111"}),
                );
                lefts.push(answer["attempts_left"].as_u64().map_or_else(
                    || answer["error"].as_str().unwrap().to_owned(),
                    |n| n.to_string(),
                ));
            }
            assert_eq!(lefts, ["4", "3", "2", "1", "too_many_attempts"]);
            let (_, right) = post(
                &mut http,
                &url,
                "/auth/email/verify",
                json!({"request_id":id,"secret":"s","code":DEFAULT_CODE}),
            );
            // The dead code holds nothing back: a new start for the same address and install is accepted at once.
            assert_eq!(
                post(
                    &mut http,
                    &url,
                    "/auth/email/start",
                    start_body("t@example.test", "s9")
                )
                .0,
                200
            );
            assert_eq!(right["error"], "too_many_attempts");

            let (status, bad) = post(
                &mut http,
                &url,
                "/auth/email/start",
                start_body("not-an-address", "s"),
            );
            assert_eq!(
                (status, bad["error"].as_str()),
                (400, Some("invalid_email"))
            );
            let mut body = start_body("u@example.test", "s");
            body["client_id"] = json!("someone-else");
            let (status, bad) = post(&mut http, &url, "/auth/email/start", body);
            assert_eq!(
                (status, bad["error"].as_str()),
                (400, Some("unknown_client"))
            );
            let mut body = start_body("u@example.test", "s");
            body["install_id"] = json!("short");
            let (status, bad) = post(&mut http, &url, "/auth/email/start", body);
            assert_eq!(
                (status, bad["error"].as_str()),
                (400, Some("bad_install_id"))
            );
            // Another install asking for the same address gets its own request; no install id is never held.
            let (status, _) = post(
                &mut http,
                &url,
                "/auth/email/start",
                start_body("t@example.test", "s"),
            );
            assert_eq!(status, 429);
            let mut other = start_body("t@example.test", "s2");
            other["install_id"] = json!("install-id-of-another-0002");
            assert_eq!(post(&mut http, &url, "/auth/email/start", other).0, 200);
            let mut none = start_body("t@example.test", "s3");
            none.as_object_mut().unwrap().remove("install_id");
            none["platform"] = json!("toaster");
            assert_eq!(
                post(&mut http, &url, "/auth/email/start", none).0,
                200,
                "an unknown platform is no error"
            );
            let (status, bad) = post(
                &mut http,
                &url,
                "/auth/email/resend",
                json!({"request_id":"nope","secret":"s"}),
            );
            assert_eq!(
                (status, bad["error"].as_str()),
                (400, Some("invalid_request"))
            );
        })
        .await
        .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_switches_make_codes_wrong_or_expired_ban_or_turn_the_path_off() {
        let cases = [
            (
                MockOptions {
                    wrong_codes: true,
                    ..MockOptions::default()
                },
                400,
                "invalid_code",
            ),
            (
                MockOptions {
                    expired_codes: true,
                    ..MockOptions::default()
                },
                400,
                "code_expired",
            ),
            (
                MockOptions {
                    banned: vec!["x@example.test".into()],
                    ..MockOptions::default()
                },
                403,
                "account_banned",
            ),
        ];
        for (options, status, code) in cases {
            let server = mock(options).await;
            let url = server.url();
            tokio::task::spawn_blocking(move || {
                let mut http = BlockingHttp::new();
                let (_, started) = post(
                    &mut http,
                    &url,
                    "/auth/email/start",
                    start_body("x@example.test", "s"),
                );
                let (got, answer) = post(
                    &mut http,
                    &url,
                    "/auth/email/verify",
                    json!({"request_id":started["request_id"],"secret":"s","code":DEFAULT_CODE}),
                );
                assert_eq!((got, answer["error"].as_str()), (status, Some(code)));
                drop(server);
            })
            .await
            .unwrap();
        }
        let server = mock(MockOptions {
            no_email: true,
            ..MockOptions::default()
        })
        .await;
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            for path in [
                "/auth/email/start",
                "/auth/email/resend",
                "/auth/email/verify",
            ] {
                let (status, _) = post(&mut http, &url, path, start_body("x@example.test", "s"));
                assert_eq!(status, 404, "{path}");
            }
            // The browser's device flow still works.
            assert!(cloud::start_device(&mut http, &url, cloud::unix_now(), INSTALL).is_ok());
        })
        .await
        .unwrap();
    }

    /// The checks of `/auth/email/verify` in the server's order, one step at a time: each later check is only reached
    /// once the earlier ones pass.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn verify_checks_request_secret_expiry_code_or_link_then_ban() {
        let server = mock(MockOptions {
            code_ttl: Duration::from_secs(1),
            resend_wait: Duration::from_secs(1),
            banned: vec!["banned@example.test".into()],
            ..MockOptions::default()
        })
        .await;
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            let start = |http: &mut BlockingHttp, email: &str| {
                let (status, started) =
                    post(http, &url, "/auth/email/start", start_body(email, "s"));
                assert_eq!(status, 200, "{started}");
                (
                    started["request_id"].clone(),
                    server.code_for(email).unwrap(),
                    server.link_for(email).unwrap(),
                )
            };
            let verify = |http: &mut BlockingHttp, id: &Value, secret: &str, key: &str, v: &str| {
                let mut body = json!({"request_id":id,"secret":secret});
                body[key] = json!(v);
                let (status, answer) = post(http, &url, "/auth/email/verify", body);
                (status, answer["error"].as_str().map(str::to_owned), answer)
            };
            let error = |http: &mut BlockingHttp, id: &Value, secret: &str, key: &str, v: &str| {
                let (status, error, _) = verify(http, id, secret, key, v);
                (status, error.unwrap_or_default())
            };
            let refused = |status: u16, error: &str| (status, error.to_owned());

            // (1) An unknown request.
            let nobody = json!("no-such-request");
            assert_eq!(
                error(&mut http, &nobody, "s", "code", DEFAULT_CODE),
                refused(400, "invalid_request")
            );

            // (4) Before any expiry: no tries left is too_many_attempts even for the right code; a wrong or replaced
            // link is link_used.
            let (worn, worn_code, _) = start(&mut http, "worn@example.test");
            for _ in 0..ATTEMPTS {
                verify(&mut http, &worn, "s", "code", "000000");
            }
            assert_eq!(
                error(&mut http, &worn, "s", "code", &worn_code),
                refused(400, "too_many_attempts")
            );
            let (linked, _, _) = start(&mut http, "link@example.test");
            assert_eq!(
                error(&mut http, &linked, "s", "link_token", "not-the-link"),
                refused(400, "link_used")
            );
            // (5) A banned account: a wrong code is only invalid_code (the ban is not reached) ...
            let (banned, banned_code, _) = start(&mut http, "banned@example.test");
            let (status, wrong, answer) = verify(&mut http, &banned, "s", "code", "000000");
            assert_eq!((status, wrong.as_deref()), (400, Some("invalid_code")));
            assert_eq!(answer["attempts_left"], 4);
            // ... the right one spends the request, then answers account_banned; the request is used up after it.
            assert_eq!(
                error(&mut http, &banned, "s", "code", &banned_code),
                refused(403, "account_banned")
            );
            assert_eq!(
                error(&mut http, &banned, "s", "code", &banned_code),
                refused(400, "invalid_request")
            );
            // (6) The right code signs in; the request is spent (1: used).
            let (good, good_code, _) = start(&mut http, "good@example.test");
            let (status, _, tokens) = verify(&mut http, &good, "s", "code", &good_code);
            assert_eq!(status, 200);
            assert!(tokens["access_token"].as_str().is_some());
            assert_eq!(
                error(&mut http, &good, "s", "code", &good_code),
                refused(400, "invalid_request")
            );

            // (3) Once a request has expired, every try answers code_expired: the right code, a wrong one (no try is
            // spent), a wrong link, and a request whose tries are used up.
            let (late, late_code, late_link) = start(&mut http, "late@example.test");
            std::thread::sleep(Duration::from_millis(1100));
            // (2) The secret is checked before the expiry: another secret is invalid_request.
            assert_eq!(
                error(&mut http, &late, "other", "code", &late_code),
                refused(400, "invalid_request")
            );
            for (key, v) in [
                ("code", late_code.as_str()),
                ("code", "000000"),
                ("link_token", "not-the-link"),
                ("link_token", late_link.as_str()),
            ] {
                assert_eq!(
                    error(&mut http, &late, "s", key, v),
                    refused(400, "code_expired"),
                    "{key} {v}"
                );
            }
            assert_eq!(
                error(&mut http, &worn, "s", "code", &worn_code),
                refused(400, "code_expired")
            );
            // A Resend gives the expired request a new code, and the tries back: the code is reached again.
            let (status, _) = post(
                &mut http,
                &url,
                "/auth/email/resend",
                json!({"request_id":late,"secret":"s"}),
            );
            assert_eq!(status, 200);
            let (_, wrong, _) = verify(&mut http, &late, "s", "code", &late_code);
            assert_eq!(
                wrong.as_deref(),
                Some("invalid_code"),
                "the old code is a wrong try now"
            );
            // The replaced link: link_used.
            assert_eq!(
                error(&mut http, &late, "s", "link_token", &late_link),
                refused(400, "link_used")
            );
            let link = server.link_for("late@example.test").unwrap();
            assert_eq!(verify(&mut http, &late, "s", "link_token", &link).0, 200);
        })
        .await
        .unwrap();
    }

    /// The per-network limits: start and resend together, every call counted (a refused one too); code checks on their
    /// own count. Over the check limit, or while the server is busy, a check spends none of the request's tries, and the
    /// same request takes the right code once the hour has passed (here: the count forgotten) or the server is free.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_network_limits_and_busy_spend_no_try() {
        let server = mock(MockOptions {
            start_per_network: 2,
            verify_per_network: 1,
            ..MockOptions::default()
        })
        .await;
        let url = server.url();
        let state = server.state.clone();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            let (status, _) = post(
                &mut http,
                &url,
                "/auth/email/start",
                start_body("not-an-address", "s"),
            );
            assert_eq!(status, 400);
            let (_, started) = post(
                &mut http,
                &url,
                "/auth/email/start",
                start_body("n@example.test", "s"),
            );
            let id = started["request_id"].clone();
            let code = server.code_for("n@example.test").unwrap();
            let raw = reqwest::blocking::Client::new()
                .post(format!("{url}/auth/email/resend"))
                .header("content-type", "application/json")
                .body(json!({"request_id":id,"secret":"s"}).to_string())
                .send()
                .unwrap();
            assert_eq!(raw.status(), 429);
            assert_eq!(raw.headers()["retry-after"], "3600");
            let refused: Value = raw.json().unwrap();
            assert_eq!(
                (refused["error"].as_str(), refused["retry_after"].as_u64()),
                (Some("too_many_requests"), Some(3600))
            );
            let verify = |http: &mut BlockingHttp, code: &str| {
                post(
                    http,
                    &url,
                    "/auth/email/verify",
                    json!({"request_id":id,"secret":"s","code":code}),
                )
            };
            let tries = || {
                state
                    .lock()
                    .unwrap()
                    .email_requests
                    .values()
                    .next()
                    .unwrap()
                    .attempts_left
            };
            assert_eq!(verify(&mut http, "000000").1["attempts_left"], 4);
            let (status, limited) = verify(&mut http, &code);
            assert_eq!(
                (status, limited["error"].as_str()),
                (429, Some("too_many_requests"))
            );
            assert_eq!(limited["retry_after"], 3600);
            assert_eq!(tries(), 4, "the network's limit spends no try");
            state.lock().unwrap().options.busy_verify = true;
            let (status, busy) = verify(&mut http, &code);
            assert_eq!((status, busy["error"].as_str()), (503, Some("busy")));
            assert_eq!(tries(), 4, "busy spends no try");
            // An hour later and the server free: the same request takes its code.
            {
                let mut state = state.lock().unwrap();
                state.options.busy_verify = false;
                state.network_verifies.clear();
            }
            assert_eq!(verify(&mut http, &code).0, 200);
        })
        .await
        .unwrap();
    }

    /// `code_already_sent` holds the same install while its code is unexpired, unspent and has tries left, with the
    /// seconds until it expires; it does not care whether a mail went out.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_same_install_is_held_while_its_code_is_usable() {
        let server = mock(MockOptions {
            code_ttl: Duration::from_secs(2),
            resend_wait: Duration::from_secs(2),
            ..MockOptions::default()
        })
        .await;
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            assert_eq!(
                post(
                    &mut http,
                    &url,
                    "/auth/email/start",
                    start_body("h@example.test", "a")
                )
                .0,
                200
            );
            let (status, held) = post(
                &mut http,
                &url,
                "/auth/email/start",
                start_body("h@example.test", "b"),
            );
            assert_eq!(
                (status, held["error"].as_str()),
                (429, Some("code_already_sent"))
            );
            assert!(
                (1..=2).contains(&held["retry_after"].as_u64().unwrap()),
                "{held}"
            );
            std::thread::sleep(Duration::from_millis(2100));
            // Expired: no longer held.
            assert_eq!(
                post(
                    &mut http,
                    &url,
                    "/auth/email/start",
                    start_body("h@example.test", "c")
                )
                .0,
                200
            );
        })
        .await
        .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_device_registers_once_per_sign_in_and_one_row_per_install() {
        let server = mock(MockOptions::default()).await;
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            let sign_in = |http: &mut BlockingHttp, secret: &str| {
                let (_, started) = post(
                    http,
                    &url,
                    "/auth/email/start",
                    start_body("one@example.test", secret),
                );
                let code = server.code_for("one@example.test").unwrap();
                let (_, tokens) = post(
                    http,
                    &url,
                    "/auth/email/verify",
                    json!({"request_id":started["request_id"],"secret":secret,"code":code}),
                );
                tokens
            };
            let register =
                |http: &mut BlockingHttp, access: Option<&str>, install: Option<&str>| {
                    let mut body =
                        json!({"public_key":"pk","name":"eBIRForms on macos","platform":"macos"});
                    if let Some(install) = install {
                        body["install_id"] = json!(install);
                    }
                    let answer = http
                        .exchange(&HttpRequest {
                            method: "POST",
                            url: format!("{url}/api/devices"),
                            content_type: "application/json",
                            bearer: access.map(str::to_owned),
                            body: body.to_string(),
                        })
                        .unwrap();
                    (
                        answer.status,
                        serde_json::from_str::<Value>(&answer.body).unwrap(),
                    )
                };
            let first = sign_in(&mut http, "one");
            let access = first["access_token"].as_str().unwrap();
            // The sign-in was asked from INSTALL: another install, or none, is a mismatch.
            assert_eq!(
                register(&mut http, Some(access), Some("install-id-of-another-0002")).1["error"],
                "install_mismatch"
            );
            assert_eq!(
                register(&mut http, Some(access), None).1["error"],
                "install_mismatch"
            );
            assert_eq!(
                register(&mut http, Some(access), Some("bad")).1["error"],
                "bad_install_id"
            );
            let (status, made) = register(&mut http, Some(access), Some(INSTALL));
            assert_eq!(status, 200);
            let (status, again) = register(&mut http, Some(access), Some(INSTALL));
            assert_eq!(
                (status, again["error"].as_str()),
                (409, Some("device_registered"))
            );
            // A refresh is the same sign-in.
            let refreshed =
                cloud::refresh_tokens(&mut http, &url, first["refresh_token"].as_str().unwrap())
                    .unwrap();
            assert_eq!(
                register(&mut http, Some(&refreshed.access_token), Some(INSTALL)).0,
                409
            );
            // A new sign-in on the same install updates its row in place: the same id.
            let second = sign_in(&mut http, "two");
            let (status, updated) =
                register(&mut http, second["access_token"].as_str(), Some(INSTALL));
            assert_eq!(status, 200);
            assert_eq!(updated["id"], made["id"]);
            assert_eq!(server.registered_devices(), 1);
            assert_eq!(
                server.registered(INSTALL),
                Some((
                    made["id"].as_str().unwrap().to_owned(),
                    "macos".into(),
                    "eBIRForms on macos".into()
                ))
            );
            assert_eq!(register(&mut http, None, Some(INSTALL)).0, 401);
        })
        .await
        .unwrap();
    }

    /// A sign-in that named no install (an older client's browser flow) may not take an install whose device belongs to
    /// another live sign-in of the account: `409 install_in_use`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_sign_in_without_an_install_cannot_take_one_in_use() {
        let server = mock(MockOptions::default()).await;
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = BlockingHttp::new();
            let register = |http: &mut BlockingHttp, access: &str| {
                http.exchange(&HttpRequest {
                    method: "POST",
                    url: format!("{url}/api/devices"),
                    content_type: "application/json",
                    bearer: Some(access.to_owned()),
                    body: json!({"public_key":"pk","name":"n","platform":"macos","install_id":INSTALL}).to_string(),
                })
                .unwrap()
            };
            // The browser account is tester@buwiz.local; its email sign-in from INSTALL registers first.
            let (_, started) = post(&mut http, &url, "/auth/email/start", start_body("tester@buwiz.local", "e"));
            let (_, tokens) = post(&mut http, &url, "/auth/email/verify", json!({"request_id":started["request_id"],"secret":"e","code":DEFAULT_CODE}));
            assert_eq!(register(&mut http, tokens["access_token"].as_str().unwrap()).status, 200);
            let started = request(&mut http, &url, "/auth/device/start", json!({"client_id":"buwiz-desktop"}).to_string());
            let started: Value = serde_json::from_str(&started.body).unwrap();
            browser_decision(&url, started["user_code"].as_str().unwrap(), "approve");
            let polled = request(&mut http, &url, "/auth/device/poll", json!({"client_id":"buwiz-desktop","device_code":started["device_code"]}).to_string());
            let polled: Value = serde_json::from_str(&polled.body).unwrap();
            let answer = register(&mut http, polled["access_token"].as_str().unwrap());
            assert_eq!(answer.status, 409);
            assert_eq!(serde_json::from_str::<Value>(&answer.body).unwrap()["error"], "install_in_use");
        })
        .await
        .unwrap();
    }

    /// The switches for what sits in front of a server: a challenge page is "not available right now", the outer
    /// limit's HTML 429 is "too many tries", on all three paths.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn html_and_an_outer_429_reach_the_client_as_their_sentences() {
        use crate::email_sign_in::{EmailError, Intent, new_secret, resend, start, verify};
        for (options, expected) in [
            (
                MockOptions {
                    html: true,
                    ..MockOptions::default()
                },
                EmailError::NotOffered,
            ),
            (
                MockOptions {
                    html_429: true,
                    ..MockOptions::default()
                },
                EmailError::Throttled,
            ),
        ] {
            let server = mock(options).await;
            let url = server.url();
            let expected = expected.clone();
            tokio::task::spawn_blocking(move || {
                let mut http = BlockingHttp::new();
                let secret = new_secret();
                assert_eq!(
                    start(
                        &mut http,
                        &url,
                        "h@example.test",
                        Intent::SignIn,
                        &secret,
                        "macos",
                        INSTALL,
                        None
                    ),
                    Err(expected.clone())
                );
                assert_eq!(
                    resend(&mut http, &url, "r", &secret, None),
                    Err(expected.clone())
                );
                assert_eq!(
                    verify(&mut http, &url, "r", &secret, "246810"),
                    Err(expected)
                );
                drop(server);
            })
            .await
            .unwrap();
        }
    }

    #[test]
    fn expiry_and_safe_browser_inputs() {
        let mut state = State::default();
        state.devices.insert(
            "expired".into(),
            Device {
                install: None,
                code: "ABCD-EFGH".into(),
                expires: Instant::now() - TTL,
                decision: None,
                consumed: false,
            },
        );
        let response = dispatch(
            &Request {
                peer: IpAddr::from([127, 0, 0, 1]),
                method: "POST".into(),
                target: "/auth/device/poll".into(),
                headers: HashMap::new(),
                body: br#"{"client_id":"buwiz-desktop","device_code":"expired"}"#.to_vec(),
            },
            &mut state,
        );
        assert_eq!(
            serde_json::from_str::<Value>(&response.body).unwrap()["error"],
            "expired_token"
        );
        assert_eq!(safe_next("//example.com"), "/device");
        assert_eq!(safe_next("/\\example.com"), "/device");
        assert_eq!(
            safe_next("/device?user_code=ABCD-EFGH"),
            "/device?user_code=ABCD-EFGH"
        );
        assert_eq!(decode_b64("e30="), Some(b"{}".to_vec()));
        assert_eq!(decode_b64("!"), None);
    }
}
