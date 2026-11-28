//! In-memory account server for development. Uses the same small Tokio HTTP stack as the mock relay.
//!
//! It speaks the device flow (`/auth/device/*`), the email-and-code sign-in (`/auth/email/start`, `/resend`, `/verify`),
//! the refresh, `/me`, logout, the device registration (`POST /api/devices`), and the account's TINs
//! (`GET /api/tax-profiles` with its `include` modes and per-sign-in limit, and the app's `POST /api/tax-profiles`, as
//! TRACK-D-CONTRACT.md has them). Every answer is the contract's; nothing is stored once it exits.

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
/// `GET /api/tax-profiles` calls one sign-in (grant) may make in [`PROFILE_READS_WINDOW`], every mode together.
pub const PROFILE_READS_PER_SIGN_IN: usize = 120;
const PROFILE_READS_WINDOW: Duration = Duration::from_secs(600);
/// `POST /api/tax-profiles` tries one sign-in may make in [`PROFILE_ADDS_WINDOW`], whatever comes of them.
pub const PROFILE_ADDS_PER_SIGN_IN: usize = 10;
const PROFILE_ADDS_WINDOW: Duration = Duration::from_secs(3600);
/// The unverified TINs an account may hold before `too_many_unverified`.
pub const MAX_UNVERIFIED: usize = 20;
/// The longest registered or trade name, in bytes after trimming.
const MAX_NAME_BYTES: usize = 200;
/// Test TIN roots: listed with `test_only`, never added from the app.
pub const TEST_ROOTS: [&str; 2] = ["000000000", "111111114"];
/// The scope of the BIR app's token: the only one that sees full TINs and may add one.
const SCOPE_APP: &str = "app";

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
    /// `GET /api/tax-profiles` calls per sign-in in 10 minutes ([`PROFILE_READS_PER_SIGN_IN`]); a test sets it low to
    /// see the `429` with `Retry-After`.
    pub tax_profile_reads: usize,
    /// `POST /api/tax-profiles` tries per sign-in in an hour ([`PROFILE_ADDS_PER_SIGN_IN`]).
    pub tax_profile_adds: usize,
    /// The account site the rows' `action` links point at; `None`: this mock's own address.
    pub account_origin: Option<String>,
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
            tax_profile_reads: PROFILE_READS_PER_SIGN_IN,
            tax_profile_adds: PROFILE_ADDS_PER_SIGN_IN,
            account_origin: None,
        }
    }
}

/// Where a TIN stands on a mock account (a live claim; ended ones are not kept).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TinState {
    /// Approved and not paused: it sends.
    Approved,
    /// Approved, paused by staff.
    Paused,
    /// Papers in review.
    Pending,
    /// Added without papers (the site's "add papers later", or the app's add).
    Unverified,
    /// Sent back for a fix: `(code, sentence)` per reason, and staff's note.
    NeedsFix {
        reasons: Vec<(String, String)>,
        note: Option<String>,
    },
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
    /// `app` for every sign-in the mock makes; another scope only through [`MockAccount::token_with_scope`].
    scope: String,
}
/// One live TIN claim of an account.
struct Claim {
    id: String,
    /// The account's user id.
    account: String,
    /// `personal` or `business`.
    kind: String,
    root: String,
    branch: String,
    registered_name: String,
    state: TinState,
    /// When it became active (seconds since the epoch), for `activated_at`.
    activated_at: Option<i64>,
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
    /// Every account's live TIN claims, oldest first.
    claims: Vec<Claim>,
    /// `GET` and `POST /api/tax-profiles` calls per sign-in (grant session), for their limits.
    profile_reads: HashMap<String, Vec<Instant>>,
    profile_adds: HashMap<String, Vec<Instant>>,
    /// Every `POST /api/tax-profiles` request, answered or refused.
    profile_posts: usize,
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
    /// A new sign-in of `email` (its account made on first use), as the BIR app's sign-in gets it: its access token.
    pub fn app_token(&self, email: &str) -> String {
        self.token_with_scope(email, SCOPE_APP)
    }
    /// A new sign-in of `email` with `scope` (another app's token, such as Books): its access token.
    pub fn token_with_scope(&self, email: &str, scope: &str) -> String {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let u = user(&mut state, &email.to_ascii_lowercase());
        let response = issue(&mut state, u, None, None);
        if let Some(grant) = state.grants.last_mut() {
            grant.scope = scope.to_owned();
        }
        let tokens: Value = serde_json::from_str(&response.body).unwrap_or_default();
        tokens["access_token"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }
    /// Put `tin` (9, 12 or 14 digits, dashes allowed) on `email`'s account in `state`, as staff or the site would; a
    /// test TIN root too. Its claim id. Panics on a TIN of another shape: a test's mistake.
    pub fn add_tin(&self, email: &str, tin: &str, state_of_tin: TinState) -> String {
        let (root, branch) = parse_combined_tin(tin).expect("a TIN of 9, 12 or 14 digits");
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let account = user(&mut state, &email.to_ascii_lowercase()).id;
        state.claims.retain(|claim| {
            !(claim.account == account && claim.root == root && claim.branch == branch)
        });
        let id = token();
        let active = matches!(state_of_tin, TinState::Approved | TinState::Paused);
        state.claims.push(Claim {
            id: id.clone(),
            account,
            kind: "personal".into(),
            root,
            branch,
            registered_name: "SYNTHETIC TAXPAYER".into(),
            state: state_of_tin,
            activated_at: active.then(unix_now),
        });
        id
    }
    /// How many `POST /api/tax-profiles` requests came in, refused or not.
    pub fn tax_profile_posts(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .profile_posts
    }
    /// The kind, TIN (`123-456-789-00000`), registered name and state of each claim on `email`'s account, oldest first.
    pub fn tins_of(&self, email: &str) -> Vec<(String, String, String, TinState)> {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(account) = state.users.get(&email.to_ascii_lowercase()) else {
            return Vec::new();
        };
        state
            .claims
            .iter()
            .filter(|claim| claim.account == account.id)
            .map(|claim| {
                (
                    claim.kind.clone(),
                    dashed_tin(&claim.root, &claim.branch),
                    claim.registered_name.clone(),
                    claim.state.clone(),
                )
            })
            .collect()
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
            201 => "Created",
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
    if let Some(held) = state.users.get(email) {
        return held.clone();
    }
    let made = User {
        id: token(),
        email: email.to_owned(),
    };
    state.users.insert(email.to_owned(), made.clone());
    // Test TINs are added by Buwiz, never from the app: a new account carries both test roots already active,
    // as production accounts do. Real roots are untouched.
    for root in TEST_ROOTS {
        state.claims.push(Claim {
            id: token(),
            account: made.id.clone(),
            kind: "personal".into(),
            root: root.into(),
            branch: "00000".into(),
            registered_name: "TEST TIN".into(),
            state: TinState::Approved,
            activated_at: Some(unix_now()),
        });
    }
    made
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
        scope: SCOPE_APP.to_owned(),
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
            let scope = old.scope.clone();
            let response = issue(state, old.user, Some(old.session), old.install);
            if let Some(grant) = state.grants.last_mut() {
                grant.scope = scope;
            }
            response
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
        ("GET", "/api/tax-profiles") => tax_profiles(r, state, &query),
        ("POST", "/api/tax-profiles") => add_tax_profile(r, state),
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

// ------------------------------------------------------------------ the account's TINs (TRACK-D-CONTRACT.md)

const PROFILE_PERMISSIONS: [&str; 3] = ["profile.view", "filing.prepare", "filing.submit"];
const OPEN_PERMISSIONS: [&str; 2] = ["profile.view", "filing.prepare"];
const VIEW_PERMISSIONS: [&str; 1] = ["profile.view"];
const PAUSED_MESSAGE: &str = "Sending is paused for this TIN. Write to support@buwiz.com.";
const NEEDS_FIX_PREFIX: &str = "Sent back for a fix: ";
const NEEDS_FIX_NO_REASON: &str =
    "Sent back for a fix. See what to change on My TINs at account.buwiz.com.";
const TEST_TIN_MESSAGE: &str = "A test TIN: a send for it ends as a dry run, with a signed receipt. Buwiz never sends it to the BIR.";
const TEST_TIN_REFUSED: &str = "This TIN is reserved for testing: only Buwiz staff add test TINs. If it is yours, write to support@buwiz.com.";

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// The API's error: `{"error": {"code": …, "message": …}}` (the account and filing API's shape, not the sign-in's).
fn api_error(status: u16, code: &str, message: &str) -> Response {
    json_response(status, json!({"error": {"code": code, "message": message}}))
}

/// A TIN with its branch in one field, as the account site reads it: 9 digits for head office, 12 or 14 with the
/// branch; anything between the digits ignored. `(root, 5-digit branch)`.
fn parse_combined_tin(raw: &str) -> Option<(String, String)> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    let branch = match digits.len() {
        9 => "00000".to_owned(),
        12 => format!("00{}", &digits[9..]),
        14 => digits[9..].to_owned(),
        _ => return None,
    };
    Some((digits[..9].to_owned(), branch))
}

fn dashed_tin(root: &str, branch: &str) -> String {
    format!("{}-{}-{}-{branch}", &root[..3], &root[3..6], &root[6..])
}

/// The grant a request's bearer token names: `(session, account, scope)`; `401 invalid_token` otherwise.
fn api_bearer(r: &Request, state: &State) -> Result<(String, String, String), Response> {
    let bearer = r
        .headers
        .get("authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or_default();
    state
        .grants
        .iter()
        .find(|g| !bearer.is_empty() && g.access == bearer && g.access_expires > Instant::now())
        .map(|g| (g.session.clone(), g.user.id.clone(), g.scope.clone()))
        .ok_or_else(|| api_error(401, "invalid_token", "sign in again"))
}

/// One limit per sign-in: `limit` calls in `window`; past it `429 rate_limited` with `Retry-After` (whole seconds
/// until the oldest call leaves the window, at least one) and the server's words. A refused call is not counted.
fn per_sign_in(
    calls: &mut HashMap<String, Vec<Instant>>,
    session: &str,
    limit: usize,
    window: Duration,
) -> Option<Response> {
    let made = calls.entry(session.to_owned()).or_default();
    made.retain(|at| at.elapsed() < window);
    if made.len() >= limit {
        let oldest = made.iter().min().copied().unwrap_or_else(Instant::now);
        let seconds = seconds_until(oldest + window).max(1);
        let minutes = seconds.div_ceil(60);
        let unit = if minutes == 1 { "minute" } else { "minutes" };
        let mut refused = api_error(
            429,
            "rate_limited",
            &format!("Too many requests. Try again in {minutes} {unit}."),
        );
        refused.headers.push(("retry-after", seconds.to_string()));
        return Some(refused);
    }
    made.push(Instant::now());
    None
}

/// The account site the rows link to: the option's, else this mock's own address.
fn account_origin(r: &Request, state: &State) -> String {
    state.options.account_origin.clone().unwrap_or_else(|| {
        let host = r
            .headers
            .get("host")
            .map(String::as_str)
            .unwrap_or("localhost:3008");
        format!("http://{host}")
    })
}

/// One claim as `include=all` lists it (the server's `listed_json`): `next` from the same standing as `status`, so
/// `next == "fix"` exactly when the BIR app's row has an `action` and `next == "send"` exactly when `can_send`. Only
/// the BIR app (`full`) sees the TIN, the filing permissions, the links and the note.
fn listed_row(claim: &Claim, full: bool, origin: &str) -> Value {
    let (status, can_send, next, message) = match &claim.state {
        TinState::Approved => (
            "approved",
            true,
            "send",
            "Active. You can send returns for this TIN.".to_owned(),
        ),
        TinState::Paused => ("paused", false, "wait", PAUSED_MESSAGE.to_owned()),
        TinState::Pending => (
            "pending",
            false,
            "wait",
            "Waiting for review, usually within one business day.".to_owned(),
        ),
        TinState::Unverified => (
            "unverified",
            false,
            "fix",
            "Not verified. Verify it with its papers to send returns.".to_owned(),
        ),
        TinState::NeedsFix { reasons, .. } if reasons.is_empty() => {
            ("needs_fix", false, "fix", NEEDS_FIX_NO_REASON.to_owned())
        }
        TinState::NeedsFix { reasons, .. } => (
            "needs_fix",
            false,
            "fix",
            format!(
                "{NEEDS_FIX_PREFIX}{}",
                reasons
                    .iter()
                    .map(|(_, sentence)| sentence.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        ),
    };
    let action = match &claim.state {
        TinState::Unverified => Some(("Verify to send", "verify")),
        TinState::NeedsFix { .. } => Some(("Fix and send again", "fix")),
        _ => None,
    };
    let permissions: &[&str] = match (full, can_send) {
        (true, true) => &PROFILE_PERMISSIONS,
        (true, false) => &OPEN_PERMISSIONS,
        (false, _) => &VIEW_PERMISSIONS,
    };
    let mut row = json!({
        "id": claim.id,
        "tin_masked": format!("***-***-{}-{}", &claim.root[6..], claim.branch),
        "registered_name": claim.registered_name,
        "activated_at": claim.activated_at.filter(|_| matches!(claim.state, TinState::Approved | TinState::Paused)),
        "permissions": permissions,
        "status": status,
        "can_send": can_send,
        "next": next,
        "message": message,
        "action": match action {
            Some((label, page)) if full => json!({"label": label, "url": format!("{origin}/tins/claims/{}/{page}", claim.id)}),
            _ => Value::Null,
        },
    });
    if full {
        row["tin"] = json!(dashed_tin(&claim.root, &claim.branch));
    }
    if TEST_ROOTS.contains(&claim.root.as_str()) {
        row["test_only"] = json!(true);
        if can_send {
            row["message"] = json!(TEST_TIN_MESSAGE);
        }
    }
    if let TinState::NeedsFix { reasons, note } = &claim.state {
        row["reasons"] = json!(
            reasons
                .iter()
                .map(|(_, sentence)| sentence)
                .collect::<Vec<_>>()
        );
        row["reason_codes"] = json!(reasons.iter().map(|(code, _)| code).collect::<Vec<_>>());
        row["note"] = json!(note.as_deref().filter(|_| full));
    }
    row
}

/// `GET /api/tax-profiles`. `include=all` (also `all,pending`, as shipped apps ask): every live claim with `next`,
/// those that send first, then paused ones, then the rest, oldest first, with `"include":"all"`. `include=pending`:
/// the ones that send, then the open ones, each with `status`. No include: the ones that send. Every mode counts
/// toward one limit per sign-in ([`MockOptions::tax_profile_reads`] in 10 minutes).
fn tax_profiles(r: &Request, state: &mut State, query: &HashMap<String, String>) -> Response {
    let (session, account, scope) = match api_bearer(r, state) {
        Ok(grant) => grant,
        Err(refused) => return refused,
    };
    let limit = state.options.tax_profile_reads;
    if let Some(refused) = per_sign_in(
        &mut state.profile_reads,
        &session,
        limit,
        PROFILE_READS_WINDOW,
    ) {
        return refused;
    }
    let full = scope == SCOPE_APP;
    let origin = account_origin(r, state);
    let include: Vec<&str> = query
        .get("include")
        .map(|include| include.split(',').map(str::trim).collect())
        .unwrap_or_default();
    let mine: Vec<&Claim> = state
        .claims
        .iter()
        .filter(|claim| claim.account == account)
        .collect();
    let group = |claim: &&Claim| match claim.state {
        TinState::Approved => 0,
        TinState::Paused => 1,
        _ => 2,
    };
    if include.contains(&"all") {
        let mut listed = mine;
        listed.sort_by_key(group);
        let profiles: Vec<Value> = listed
            .into_iter()
            .map(|claim| listed_row(claim, full, &origin))
            .collect();
        return json_response(200, json!({"include": "all", "profiles": profiles}));
    }
    let older = |claim: &Claim, status: Option<&str>| {
        let mut row = json!({
            "id": claim.id,
            "tin_masked": format!("***-***-{}-{}", &claim.root[6..], claim.branch),
            "registered_name": claim.registered_name,
            "activated_at": claim.activated_at,
            "permissions": if full { &PROFILE_PERMISSIONS[..] } else { &VIEW_PERMISSIONS[..] },
        });
        if full {
            row["tin"] = json!(dashed_tin(&claim.root, &claim.branch));
        }
        if let Some(status) = status {
            row["status"] = json!(status);
        }
        if TEST_ROOTS.contains(&claim.root.as_str()) {
            row["test_only"] = json!(true);
        }
        row
    };
    let pending = include.contains(&"pending");
    let mut profiles: Vec<Value> = mine
        .iter()
        .filter(|claim| claim.state == TinState::Approved)
        .map(|claim| older(claim, pending.then_some("approved")))
        .collect();
    if pending {
        profiles.extend(mine.iter().filter_map(|claim| {
            let status = match claim.state {
                TinState::Pending => "pending",
                TinState::NeedsFix { .. } => "needs_fix",
                TinState::Unverified => "unverified",
                _ => return None,
            };
            Some(older(claim, Some(status)))
        }));
    }
    json_response(200, json!({"profiles": profiles}))
}

/// `POST /api/tax-profiles`, the server's checks in its order: the BIR app's token only (`403 insufficient_scope`),
/// then the per-sign-in limit (`429 rate_limited`), the JSON and its `kind` (`bad_request`), the TIN (`bad_tin`), a test
/// root (`test_tin`), the names (`name_missing` for an empty registered name, `name_too_long` past 200 bytes), what this
/// account already holds (`409 tin_already_active`, `tin_already_added`, `claim_already_pending`) and the cap of
/// unverified TINs (`409 too_many_unverified`). Then the row is saved as not verified: `201 {"profile": row}`.
fn add_tax_profile(r: &Request, state: &mut State) -> Response {
    state.profile_posts += 1;
    let (session, account, scope) = match api_bearer(r, state) {
        Ok(grant) => grant,
        Err(refused) => return refused,
    };
    if scope != SCOPE_APP {
        return api_error(
            403,
            "insufficient_scope",
            "only the Buwiz BIR app can file returns",
        );
    }
    let limit = state.options.tax_profile_adds;
    if let Some(refused) = per_sign_in(
        &mut state.profile_adds,
        &session,
        limit,
        PROFILE_ADDS_WINDOW,
    ) {
        return refused;
    }
    let bad = || {
        api_error(
            400,
            "bad_request",
            "Send a JSON object with kind, tin and registered_name.",
        )
    };
    let Ok(Value::Object(add)) = serde_json::from_slice::<Value>(&r.body) else {
        return bad();
    };
    let text = |key: &str| match add.get(key) {
        Some(Value::String(text)) => Some(Some(text.as_str())),
        None | Some(Value::Null) => Some(None),
        Some(_) => None,
    };
    let (Some(Some(kind)), Some(Some(tin)), Some(Some(registered_name)), Some(trade_name)) = (
        text("kind"),
        text("tin"),
        text("registered_name"),
        text("trade_name"),
    ) else {
        return bad();
    };
    if !matches!(kind, "personal" | "business") {
        return api_error(
            400,
            "bad_request",
            "Choose whether this is a personal or a business TIN.",
        );
    }
    let Some((root, branch)) = parse_combined_tin(tin) else {
        return api_error(400, "bad_tin", "The TIN must have 9 digits.");
    };
    if TEST_ROOTS.contains(&root.as_str()) {
        return api_error(400, "test_tin", TEST_TIN_REFUSED);
    }
    let registered_name = registered_name.trim();
    let trade_name = trade_name.unwrap_or_default().trim();
    if registered_name.is_empty() {
        return api_error(
            400,
            "name_missing",
            "Enter the registered name exactly as it appears on your document.",
        );
    }
    if registered_name.len() > MAX_NAME_BYTES || trade_name.len() > MAX_NAME_BYTES {
        return api_error(
            400,
            "name_too_long",
            "The registered name and the trade name can each be 200 bytes at most.",
        );
    }
    let held = state
        .claims
        .iter()
        .find(|claim| claim.account == account && claim.root == root && claim.branch == branch);
    match held.map(|claim| &claim.state) {
        Some(TinState::Approved | TinState::Paused) => {
            return api_error(
                409,
                "tin_already_active",
                "This TIN is already active on your account.",
            );
        }
        Some(TinState::Unverified) => {
            return api_error(
                409,
                "tin_already_added",
                "This TIN is already on My TINs. Verify it there to send returns.",
            );
        }
        Some(_) => {
            return api_error(
                409,
                "claim_already_pending",
                "You already have a claim for this TIN waiting for review.",
            );
        }
        None => {}
    }
    let waiting = state
        .claims
        .iter()
        .filter(|claim| claim.account == account && claim.state == TinState::Unverified)
        .count();
    if waiting >= MAX_UNVERIFIED {
        return api_error(
            409,
            "too_many_unverified",
            &format!(
                "You have {MAX_UNVERIFIED} TINs waiting to be verified. Verify or remove some first."
            ),
        );
    }
    let claim = Claim {
        id: token(),
        account,
        kind: kind.to_owned(),
        root,
        branch,
        registered_name: registered_name.to_owned(),
        state: TinState::Unverified,
        activated_at: None,
    };
    let origin = account_origin(r, state);
    let profile = listed_row(&claim, true, &origin);
    state.claims.push(claim);
    json_response(201, json!({"profile": profile}))
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
    fn a_new_account_carries_the_test_roots_approved() {
        // Test TINs are added by Buwiz, never from the app: every account the mock creates carries both test roots
        // active and test_only (head office), as production accounts do. A real root is never seeded.
        let state: Arc<Mutex<State>> = Arc::new(Mutex::new(State::default()));
        {
            let mut held = state.lock().unwrap_or_else(|e| e.into_inner());
            user(&mut held, "fresh@example.test");
        }
        let held = state.lock().unwrap_or_else(|e| e.into_inner());
        let account = held.users["fresh@example.test"].id.clone();
        let rows: Vec<(&str, &str, &TinState)> = held
            .claims
            .iter()
            .filter(|claim| claim.account == account)
            .map(|claim| (claim.root.as_str(), claim.branch.as_str(), &claim.state))
            .collect();
        assert_eq!(
            rows.len(),
            2,
            "exactly the two test roots, nothing else: {rows:?}"
        );
        for root in TEST_ROOTS {
            let row = rows
                .iter()
                .find(|(r, _, _)| *r == root)
                .expect("the test root is seeded");
            assert!(
                matches!(row.2, TinState::Approved),
                "the test root is active: {row:?}"
            );
            assert_eq!(row.1, "00000", "the head office");
        }
        assert!(
            !rows.iter().any(|(r, _, _)| *r == "123456788"),
            "a real root is never seeded"
        );
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

    /// The account's TINs (TRACK-D-CONTRACT.md) through the app's own client, `bir_submit::Submitter`.
    mod tax_profiles {
        use std::sync::Arc;

        use bir_submit::{AddTinError, Next, SubmitConfig, Submitter, TinKey, TinKind, TinsError};

        use super::*;

        const EMAIL: &str = "filer@example.test";

        /// A fixed access token, as the host's sign-in would give it.
        struct Fixed(String);
        impl bir_submit::TokenSource for Fixed {
            fn access_token(
                &self,
                _force_refresh: bool,
            ) -> std::pin::Pin<
                Box<
                    dyn std::future::Future<Output = Result<String, bir_submit::ClientError>>
                        + Send
                        + '_,
                >,
            > {
                let token = self.0.clone();
                Box::pin(async move { Ok(token) })
            }
            fn login(&self) -> Option<String> {
                Some(EMAIL.into())
            }
            fn session_ended(&self) {}
        }

        fn client(url: &str, token: &str) -> Submitter {
            Submitter::new(
                SubmitConfig::new(url).unwrap(),
                Box::new(bir_submit::secrets::MemoryStore::default()),
                Arc::new(Fixed(token.to_owned())),
            )
        }

        /// A mock whose rows link to the app's account site, so their links survive `pinned_account_url`.
        async fn server(options: MockOptions) -> MockAccount {
            mock(MockOptions {
                account_origin: Some(bir_submit::tins::account_site()),
                ..options
            })
            .await
        }

        async fn raw(
            method: reqwest::Method,
            url: &str,
            token: &str,
            body: Option<Value>,
        ) -> (u16, Option<String>, Value) {
            let mut request = reqwest::Client::new()
                .request(method, url)
                .bearer_auth(token);
            if let Some(body) = body {
                request = request.json(&body);
            }
            let response = request.send().await.unwrap();
            let retry_after = response
                .headers()
                .get("retry-after")
                .map(|value| value.to_str().unwrap().to_owned());
            (
                response.status().as_u16(),
                retry_after,
                response.json().await.unwrap_or_default(),
            )
        }

        fn key(tin: &str) -> TinKey {
            TinKey::parse(tin).unwrap()
        }

        /// One TIN in every state, and two test TINs. Each row's `next` agrees with `action` and `can_send` (the
        /// contract's invariants), on the raw answer and as the client reads it; test TINs read with `test_only`.
        /// Another app's token gets the same `next` and never an action link.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn next_invariants_hold_on_the_mock() {
            let server = server(MockOptions::default()).await;
            let fix = vec![(
                "blurry".to_owned(),
                "A photo is too blurry to read.".to_owned(),
            )];
            for (tin, state) in [
                ("000-123-453-00000", TinState::Approved),
                ("000-123-454-00000", TinState::Paused),
                ("000-123-455-00000", TinState::Unverified),
                ("000-123-456-00000", TinState::Pending),
                (
                    "000-123-457-00000",
                    TinState::NeedsFix {
                        reasons: fix.clone(),
                        note: Some("Retake it in daylight.".into()),
                    },
                ),
                (
                    "000-123-457-00000",
                    TinState::NeedsFix {
                        reasons: fix,
                        note: None,
                    },
                ),
                ("000-000-000-00000", TinState::Approved),
                ("111-111-114-00001", TinState::Pending),
            ] {
                server.add_tin(EMAIL, tin, state);
            }
            let token = server.app_token(EMAIL);
            let url = format!("{}/api/tax-profiles?include=all", server.url());
            let (status, _, body) = raw(reqwest::Method::GET, &url, &token, None).await;
            assert_eq!(status, 200);
            assert_eq!(body["include"], "all");
            let rows = body["profiles"].as_array().unwrap();
            assert_eq!(rows.len(), 8);
            for row in rows {
                let next = row["next"].as_str().unwrap();
                assert_eq!(next == "fix", !row["action"].is_null(), "{row}");
                assert_eq!(next == "send", row["can_send"] == true, "{row}");
                let test = ["000-000-000", "111-111-114"]
                    .iter()
                    .any(|root| row["tin"].as_str().unwrap().starts_with(root));
                assert_eq!(row["test_only"] == true, test, "{row}");
            }
            // Those that send first, then paused, then the rest oldest first.
            let statuses: Vec<&str> = rows
                .iter()
                .map(|row| row["status"].as_str().unwrap())
                .collect();
            assert_eq!(
                statuses,
                [
                    "approved",
                    "approved",
                    "approved",
                    "paused",
                    "unverified",
                    "pending",
                    "needs_fix",
                    "pending"
                ]
            );

            let list = client(&server.url(), &token).account_tins(0).await.unwrap();
            assert!(list.complete && !list.stale);
            assert_eq!(list.rows.len(), 8);
            for row in &list.rows {
                assert_eq!(row.next == Next::Fix, row.action_url.is_some(), "{row:?}");
                assert_eq!(row.next == Next::Send, row.can_send, "{row:?}");
                assert!(row.message.is_some(), "{row:?}");
            }
            let test = list.find(&key("000000000")).unwrap();
            assert!(test.test_only && test.can_send && test.next == Next::Send);
            assert!(list.find(&key("111-111-114-001")).unwrap().test_only);
            let fixing = list.find(&key("000123457")).unwrap();
            assert_eq!(fixing.reasons, ["A photo is too blurry to read."]);
            assert_eq!(fixing.note, None, "the later claim replaced the first");
            assert!(fixing.action_url.as_deref().unwrap().ends_with("/fix"));
            assert!(
                list.find(&key("000123455"))
                    .unwrap()
                    .action_url
                    .as_deref()
                    .unwrap()
                    .ends_with("/verify")
            );

            let books = server.token_with_scope(EMAIL, "books");
            let (_, _, other) = raw(reqwest::Method::GET, &url, &books, None).await;
            for (row, app) in other["profiles"].as_array().unwrap().iter().zip(rows) {
                assert_eq!(row["next"], app["next"]);
                assert!(row["action"].is_null() && row.get("tin").is_none(), "{row}");
            }
            // The older modes have no next.
            for older in ["", "?include=pending"] {
                let (_, _, body) = raw(
                    reqwest::Method::GET,
                    &format!("{}/api/tax-profiles{older}", server.url()),
                    &token,
                    None,
                )
                .await;
                let rows = body["profiles"].as_array().unwrap();
                assert!(!rows.is_empty() && rows.iter().all(|row| row.get("next").is_none()));
            }
        }

        /// Past the read limit the mock answers `429 rate_limited` with `Retry-After`; the client keeps its list and
        /// asks nothing more until then. Another sign-in has its own limit.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn the_read_limit_answers_429_and_the_client_keeps_its_list() {
            let server = server(MockOptions {
                tax_profile_reads: 2,
                ..MockOptions::default()
            })
            .await;
            server.add_tin(EMAIL, "000123453", TinState::Approved);
            let token = server.app_token(EMAIL);
            let app = client(&server.url(), &token);
            let first = app.account_tins(0).await.unwrap();
            let url = format!("{}/api/tax-profiles", server.url());
            let (status, retry_after, body) = raw(reqwest::Method::GET, &url, &token, None).await;
            assert_eq!(status, 200, "{body}");
            let (status, retry_after_2, body) = raw(reqwest::Method::GET, &url, &token, None).await;
            assert_eq!(retry_after, None);
            assert_eq!(status, 429);
            assert_eq!(body["error"]["code"], "rate_limited");
            assert_eq!(
                body["error"]["message"],
                "Too many requests. Try again in 10 minutes."
            );
            let seconds: u64 = retry_after_2.unwrap().parse().unwrap();
            assert!((590..=600).contains(&seconds), "{seconds}");

            let kept = app.account_tins(0).await.unwrap();
            assert!(kept.stale);
            assert_eq!(kept.rows, first.rows);
            assert!(app.account_tins(0).await.unwrap().stale);
            app.invalidate_account_tins();
            assert!(matches!(
                app.account_tins(0).await,
                Err(TinsError::RateLimited { .. })
            ));

            let again = server.app_token(EMAIL);
            let (status, _, _) = raw(reqwest::Method::GET, &url, &again, None).await;
            assert_eq!(status, 200, "a sign-in of its own");
        }

        /// What the app adds is saved as not verified, under its kind and 14-digit TIN, and lists as Fix with its
        /// Verify link.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn an_add_from_the_app_lists_as_fix() {
            let server = server(MockOptions::default()).await;
            let app = client(&server.url(), &server.app_token(EMAIL));
            let added = app
                .add_account_tin(
                    TinKind::NonIndividual,
                    &key("000123453002"),
                    " Dela Cruz Corp ",
                    None,
                )
                .await
                .unwrap()
                .unwrap();
            assert_eq!(added.next, Next::Fix);
            assert!(added.action_url.as_deref().unwrap().ends_with("/verify"));
            app.add_account_tin(
                TinKind::Individual,
                &key("000123460"),
                "Dela Cruz Juan",
                Some("Juan's"),
            )
            .await
            .unwrap();
            assert_eq!(
                server.tins_of(EMAIL),
                [
                    (
                        "personal".to_owned(),
                        "000-000-000-00000".to_owned(),
                        "TEST TIN".to_owned(),
                        TinState::Approved
                    ),
                    (
                        "personal".to_owned(),
                        "111-111-114-00000".to_owned(),
                        "TEST TIN".to_owned(),
                        TinState::Approved
                    ),
                    (
                        "business".to_owned(),
                        "000-123-453-00002".to_owned(),
                        "Dela Cruz Corp".to_owned(),
                        TinState::Unverified
                    ),
                    (
                        "personal".to_owned(),
                        "000-123-460-00000".to_owned(),
                        "Dela Cruz Juan".to_owned(),
                        TinState::Unverified
                    ),
                ]
            );
            let list = app.account_tins(60).await.unwrap();
            assert_eq!(list.find(&key("000123453002")).unwrap().next, Next::Fix);
            assert_eq!(server.tax_profile_posts(), 2);
        }

        /// Every refusal of `POST /api/tax-profiles`, each set up on the mock, reaches the app as its own case.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn the_mock_refuses_each_add_with_its_code() {
            let server = server(MockOptions {
                tax_profile_adds: 100,
                ..MockOptions::default()
            })
            .await;
            server.add_tin(EMAIL, "000123453", TinState::Approved);
            server.add_tin(EMAIL, "000123454", TinState::Paused);
            server.add_tin(EMAIL, "000123455", TinState::Unverified);
            server.add_tin(EMAIL, "000123456", TinState::Pending);
            server.add_tin(
                EMAIL,
                "000123457",
                TinState::NeedsFix {
                    reasons: Vec::new(),
                    note: None,
                },
            );
            let token = server.app_token(EMAIL);
            let app = client(&server.url(), &token);
            let add = |tin: &'static str| {
                let app = &app;
                async move {
                    app.add_account_tin(TinKind::Individual, &key(tin), "Dela Cruz Juan", None)
                        .await
                }
            };
            assert_eq!(add("000123453").await, Err(AddTinError::TinAlreadyActive));
            assert_eq!(add("000123454").await, Err(AddTinError::TinAlreadyActive));
            assert_eq!(add("000123455").await, Err(AddTinError::TinAlreadyAdded));
            assert_eq!(
                add("000123456").await,
                Err(AddTinError::ClaimAlreadyPending)
            );
            assert_eq!(
                add("000123457").await,
                Err(AddTinError::ClaimAlreadyPending)
            );
            assert_eq!(add("000000000").await, Err(AddTinError::TestTin));
            assert_eq!(add("111111114").await, Err(AddTinError::TestTin));
            // Another account's TIN reads like a free one.
            server.add_tin("other@example.test", "000123458", TinState::Approved);
            assert!(add("000123458").await.is_ok());

            // What the client never sends: the server's own checks.
            let url = format!("{}/api/tax-profiles", server.url());
            let post = |body: Value| raw(reqwest::Method::POST, &url, &token, Some(body));
            let named = "Ñ".repeat(150);
            for (body, code) in [
                (
                    json!({"kind":"personal","tin":"000-123-46","registered_name":"X"}),
                    "bad_tin",
                ),
                (
                    json!({"kind":"personal","tin":"000123461","registered_name":"  "}),
                    "name_missing",
                ),
                (
                    json!({"kind":"personal","tin":"000123461","registered_name":named}),
                    "name_too_long",
                ),
                (
                    json!({"kind":"personal","tin":"000123461","registered_name":"X","trade_name":named}),
                    "name_too_long",
                ),
                (
                    json!({"kind":"partnership","tin":"000123461","registered_name":"X"}),
                    "bad_request",
                ),
                (
                    json!({"tin":"000123461","registered_name":"X"}),
                    "bad_request",
                ),
                (json!(["not", "an", "object"]), "bad_request"),
            ] {
                let (status, _, answer) = post(body.clone()).await;
                assert_eq!(
                    (status, answer["error"]["code"].as_str()),
                    (400, Some(code)),
                    "{body}"
                );
            }

            // Another app's token: 403, as `require_app` answers.
            let books = client(&server.url(), &server.token_with_scope(EMAIL, "books"));
            assert_eq!(
                books
                    .add_account_tin(TinKind::Individual, &key("000123462"), "X", None)
                    .await,
                Err(AddTinError::Forbidden)
            );

            // The cap of unverified TINs.
            let crowded = "crowded@example.test";
            for n in 0..MAX_UNVERIFIED {
                server.add_tin(crowded, &format!("000200{n:03}"), TinState::Unverified);
            }
            let full = client(&server.url(), &server.app_token(crowded));
            let refused = full
                .add_account_tin(TinKind::Individual, &key("000300000"), "X", None)
                .await
                .unwrap_err();
            assert_eq!(refused, AddTinError::TooManyUnverified);
            assert_eq!(
                refused.to_string(),
                "Your account has 20 TINs waiting to be verified. Verify or remove one on your account first."
            );
        }

        /// Past 10 adds a sign-in is refused `429 rate_limited` with `Retry-After`, whatever came of the tries.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn the_add_limit_answers_429_with_retry_after() {
            let server = server(MockOptions::default()).await;
            server.add_tin(EMAIL, "000123453", TinState::Approved);
            let app = client(&server.url(), &server.app_token(EMAIL));
            for _ in 0..PROFILE_ADDS_PER_SIGN_IN {
                assert_eq!(
                    app.add_account_tin(TinKind::Individual, &key("000123453"), "X", None)
                        .await,
                    Err(AddTinError::TinAlreadyActive)
                );
            }
            let refused = app
                .add_account_tin(TinKind::Individual, &key("000123460"), "X", None)
                .await
                .unwrap_err();
            let AddTinError::RateLimited { retry_after } = refused else {
                panic!("{refused:?}");
            };
            assert!((3590..=3600).contains(&retry_after), "{retry_after}");
            assert_eq!(
                refused.to_string(),
                "Too many tries. Try again in 60 minutes."
            );
            assert!(
                server.tins_of(EMAIL).len() == 3,
                "nothing was added past the two seeded test roots and the one add"
            );
        }
    }
}
