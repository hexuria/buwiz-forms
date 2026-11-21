//! In-memory account server for development. Uses the same small Tokio HTTP stack as the mock relay.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(900);
const DEFAULT_EMAIL: &str = "dev@buwiz.local";

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
    access: String,
    access_expires: Instant,
    refresh: String,
    refresh_expires: Instant,
}
#[derive(Default)]
struct State {
    devices: HashMap<String, Device>,
    users: HashMap<String, User>,
    sessions: HashMap<String, User>,
    grants: Vec<Grant>,
}

/// A running account server. Dropping it closes the listener.
pub struct MockAccount {
    addr: SocketAddr,
    task: tokio::task::JoinHandle<()>,
}
impl MockAccount {
    pub async fn start(addr: SocketAddr, auto_approve: bool) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        let state = Arc::new(Mutex::new(State::default()));
        let task = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let state = state.clone();
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
                                    auto_approve,
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
        Ok(Self { addr, task })
    }
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}
impl Drop for MockAccount {
    fn drop(&mut self) {
        self.task.abort();
    }
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
            404 => "Not Found",
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
fn issue(state: &mut State, user: User) -> Response {
    let access = token();
    let refresh = token();
    state.grants.push(Grant {
        user,
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

fn dispatch(r: &Request, state: &mut State, auto: bool) -> Response {
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
                    issue(state, user)
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
            issue(state, old.user)
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
        _ => oauth(404, "not_found", "no such endpoint"),
    }
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
            let (id, email) = cloud::load_profile(&mut http, &url, &tokens.access_token).unwrap();
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
            false,
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
