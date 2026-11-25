//! In-memory account server for development. Uses the same small Tokio HTTP stack as the mock relay.
//!
//! It speaks the device flow (`/auth/device/*`), the email-and-code sign-in (`/auth/email/start`, `/resend`, `/verify`),
//! the refresh, `/me`, logout, and the device registration (`POST /api/devices`). Every answer is the contract's;
//! nothing is stored once it exits.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(900);
const DEFAULT_EMAIL: &str = "dev@buwiz.local";
/// The code every email sign-in gets unless the options say otherwise.
pub const DEFAULT_CODE: &str = "246810";
/// Wrong codes a request takes before it answers `too_many_attempts`.
const ATTEMPTS: u32 = 5;
const PLATFORMS: [&str; 6] = ["ios", "ipados", "android", "macos", "windows", "linux"];

/// How the mock answers the email sign-in.
#[derive(Clone, Debug)]
pub struct MockOptions {
    /// Approve device codes at once as [`DEFAULT_EMAIL`].
    pub auto_approve: bool,
    /// Every request's code; `None`: a new random code per request (printed, and [`MockAccount::code_for`]).
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
        }
    }
}

#[derive(Clone)]
struct User {
    id: String,
    email: String,
}
struct Device {
    code: String,
    expires: Instant,
    decision: Option<Option<User>>,
    consumed: bool,
}
struct Grant {
    user: User,
    /// The sign-in this grant belongs to; a refresh keeps it.
    session: String,
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
    /// Links of earlier codes of this request (after a Resend): `link_used`.
    old_links: Vec<String>,
    expires: Instant,
    resend_at: Instant,
    attempts_left: u32,
    spent: bool,
}
/// A registered device: one row per install id.
struct DeviceRow {
    id: String,
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
    registered: HashMap<String, DeviceRow>,
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
            while let Ok((mut stream, _)) = listener.accept().await {
                let state = shared.clone();
                tokio::spawn(async move {
                    let work = async {
                        let response = match read_request(&mut stream).await {
                            Ok(request) => {
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
    /// The link token of the newest live email request for `email`.
    pub fn link_for(&self, email: &str) -> Option<String> {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        newest_request(&state, email).map(|(_, request)| request.link.clone())
    }
    /// How many devices are registered (one row per install id).
    pub fn registered_devices(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .registered
            .len()
    }
    /// The platform and name a device registered with, by install id.
    pub fn registered(&self, install_id: &str) -> Option<(String, String)> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .registered
            .get(install_id)
            .map(|row| (row.platform.clone(), row.name.clone()))
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
fn issue(state: &mut State, user: User, session: Option<String>) -> Response {
    let access = token();
    let refresh = token();
    state.grants.push(Grant {
        user,
        session: session.unwrap_or_else(token),
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
                    issue(state, user, None)
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
            issue(state, old.user, Some(old.session))
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
        ("POST", "/auth/email/start") => email_start(state, &value),
        ("POST", "/auth/email/resend") => email_resend(state, &value),
        ("POST", "/auth/email/verify") => email_verify(state, &value),
        ("POST", "/api/devices") => register_device(r, state, &value),
        _ => oauth(404, "not_found", "no such endpoint"),
    }
}

/// The contract's error: `{"error": code, "message": sentence}` and whatever else the code carries.
fn failure(status: u16, code: &str, message: &str, extra: Value) -> Response {
    let mut body = json!({"error": code, "message": message});
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            body.insert(key.clone(), value.clone());
        }
    }
    json_response(status, body)
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

fn new_code(options: &MockOptions) -> String {
    options.code.clone().unwrap_or_else(|| {
        let n = u32::from_le_bytes(rand::random::<[u8; 4]>()) % 1_000_000;
        format!("{n:06}")
    })
}

/// A request's code and link start over: a new pair, a new life, Resend locked again.
fn reissue(options: &MockOptions, request: &mut EmailRequest) {
    let old = std::mem::replace(&mut request.link, token());
    request.old_links.push(old);
    request.code = new_code(options);
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
    if get("client_id") != "buwiz-desktop" {
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
    if !matches!(get("intent"), "sign_in" | "create")
        || get("secret_hash").len() != 43
        || !PLATFORMS.contains(&get("platform"))
        || !valid_install_id(get("install_id"))
    {
        return failure(
            400,
            "invalid_request",
            "The sign-in request is incomplete.",
            json!({}),
        );
    }
    if let Some((_, live)) = newest_request(state, &email)
        && live.expires > Instant::now()
    {
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
    let options = state.options.clone();
    let request_id = token();
    let request = EmailRequest {
        email,
        secret_hash: get("secret_hash").to_owned(),
        code: new_code(&options),
        link: token(),
        old_links: Vec::new(),
        expires: Instant::now() + options.code_ttl,
        resend_at: Instant::now() + options.resend_wait,
        attempts_left: ATTEMPTS,
        spent: false,
    };
    let answer = email_answer(&options, &request_id, &request);
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
    if request.resend_at > Instant::now() {
        let wait = seconds_until(request.resend_at);
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
    reissue(&options, request);
    email_answer(&options, &id, request)
}

fn email_verify(state: &mut State, value: &Value) -> Response {
    let options = state.options.clone();
    let get = |k: &str| value.get(k).and_then(Value::as_str).unwrap_or_default();
    let (code, link) = (get("code").to_owned(), get("link_token").to_owned());
    let (_, request) = match own_request(state, value) {
        Ok(found) => found,
        Err(response) => return response,
    };
    if !link.is_empty() && request.old_links.contains(&link) {
        return failure(
            400,
            "link_used",
            "This sign-in link was already used.",
            json!({}),
        );
    }
    if request.attempts_left == 0 {
        return failure(
            400,
            "too_many_attempts",
            "Too many wrong codes. Wait for a new code.",
            json!({}),
        );
    }
    if options.expired_codes || request.expires <= Instant::now() {
        return failure(
            400,
            "code_expired",
            "This code has expired. Get a new one.",
            json!({}),
        );
    }
    let matches = if link.is_empty() {
        !options.wrong_codes && code == request.code
    } else {
        link == request.link
    };
    if !matches {
        request.attempts_left -= 1;
        if request.attempts_left == 0 {
            return failure(
                400,
                "too_many_attempts",
                "Too many wrong codes. Wait for a new code.",
                json!({}),
            );
        }
        return failure(
            400,
            "invalid_code",
            "That code is not right.",
            json!({"attempts_left": request.attempts_left}),
        );
    }
    request.spent = true;
    let email = request.email.clone();
    if options
        .banned
        .iter()
        .any(|banned| banned.eq_ignore_ascii_case(&email))
    {
        return failure(403, "account_banned", "This account is closed.", json!({}));
    }
    let u = user(state, &email);
    issue(state, u, None)
}

/// `POST /api/devices` with the bearer token: one row per install id; a second registration in the same sign-in is
/// `409 device_registered` (with the id it has).
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
    let session = grant.session.clone();
    let get = |k: &str| value.get(k).and_then(Value::as_str).unwrap_or_default();
    let install_id = get("install_id").to_owned();
    if get("public_key").is_empty()
        || !valid_install_id(&install_id)
        || !PLATFORMS.contains(&get("platform"))
    {
        return failure(
            400,
            "invalid_request",
            "The device registration is incomplete.",
            json!({}),
        );
    }
    if let Some(row) = state.registered.get(&install_id)
        && row.session == session
    {
        return failure(
            409,
            "device_registered",
            "This device is already registered.",
            json!({"id": row.id}),
        );
    }
    let id = token();
    state.registered.insert(
        install_id,
        DeviceRow {
            id: id.clone(),
            session,
            platform: get("platform").to_owned(),
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
            let challenge = cloud::start_device(&mut http, &url, cloud::unix_now()).unwrap();
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
            let challenge = cloud::start_device(&mut http, &url, cloud::unix_now()).unwrap();
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
            let challenge = cloud::start_device(&mut http, &url, cloud::unix_now()).unwrap();
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

    fn random_codes() -> MockOptions {
        MockOptions {
            code: None,
            ..MockOptions::default()
        }
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
        let server = mock(random_codes()).await;
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
            if code_a == code_b {
                return; // one in a million: nothing to tell apart
            }
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
            code: None,
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
                (429, Some("code_already_sent"))
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
            if new_code != old_code {
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
            }
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
                (400, Some("invalid_request"))
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
            assert!(cloud::start_device(&mut http, &url, cloud::unix_now()).is_ok());
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
                let (_, started) = post(http, &url, "/auth/email/start", start_body(&format!("{secret}@example.test"), secret));
                let (_, tokens) = post(http, &url, "/auth/email/verify", json!({"request_id":started["request_id"],"secret":secret,"code":DEFAULT_CODE}));
                tokens
            };
            let register = |http: &mut BlockingHttp, access: Option<&str>, install: &str| {
                let answer = http
                    .exchange(&HttpRequest {
                        method: "POST",
                        url: format!("{url}/api/devices"),
                        content_type: "application/json",
                        bearer: access.map(str::to_owned),
                        body: json!({"public_key":"pk","name":"eBIRForms on macos","install_id":install,"platform":"macos"}).to_string(),
                    })
                    .unwrap();
                (answer.status, serde_json::from_str::<Value>(&answer.body).unwrap())
            };
            let first = sign_in(&mut http, "one");
            let access = first["access_token"].as_str().unwrap();
            let (status, made) = register(&mut http, Some(access), INSTALL);
            assert_eq!(status, 200);
            let (status, again) = register(&mut http, Some(access), INSTALL);
            assert_eq!((status, again["error"].as_str()), (409, Some("device_registered")));
            assert_eq!(again["id"], made["id"]);
            // A refresh is the same sign-in.
            let refreshed = cloud::refresh_tokens(&mut http, &url, first["refresh_token"].as_str().unwrap()).unwrap();
            assert_eq!(register(&mut http, Some(&refreshed.access_token), INSTALL).0, 409);
            // A new sign-in on the same install replaces the row.
            let second = sign_in(&mut http, "two");
            let (status, replaced) = register(&mut http, second["access_token"].as_str(), INSTALL);
            assert_eq!(status, 200);
            assert_ne!(replaced["id"], made["id"]);
            assert_eq!(server.registered_devices(), 1);
            assert_eq!(server.registered(INSTALL), Some(("macos".into(), "eBIRForms on macos".into())));
            assert_eq!(register(&mut http, None, INSTALL).0, 401);
            assert_eq!(register(&mut http, second["access_token"].as_str(), "bad").0, 400);
        })
        .await
        .unwrap();
    }

    #[test]
    fn expiry_and_safe_browser_inputs() {
        let mut state = State::default();
        state.devices.insert(
            "expired".into(),
            Device {
                code: "ABCD-EFGH".into(),
                expires: Instant::now() - TTL,
                decision: None,
                consumed: false,
            },
        );
        let response = dispatch(
            &Request {
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
