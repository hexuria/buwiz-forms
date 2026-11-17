//! MCP Streamable HTTP transport on loopback, for clients that refuse stdio
//! (OpenGrok). It is a second door onto the same [`Server::handle`]: one
//! dispatch, one closed tool allow-list, whichever transport carries it.
//!
//! What it serves, and why each answer is what the client needs. The client
//! this was matched against is rmcp 3.1.4's `StreamableHttpClientTransport`
//! over reqwest, which is what OpenGrok dials plugin servers with
//! (`opengrok-tools/src/mcp.rs`, `().serve(transport)`, i.e. the legacy
//! `initialize` lifecycle):
//!
//! - `POST /mcp` with one JSON-RPC message. A request gets `200` +
//!   `application/json` with the single response: rmcp accepts a JSON body as
//!   well as SSE (`common/reqwest/streamable_http_client.rs:306-318`).
//! - A notification or a client response gets `202 Accepted`, empty body
//!   (`common/reqwest/streamable_http_client.rs:244-249`).
//! - No `Mcp-Session-Id` is issued: the server is stateless. rmcp's
//!   `allow_stateless` defaults to `true` (`streamable_http_client.rs:1777`)
//!   and a missing session id on the initialize response is accepted when it
//!   is (`streamable_http_client.rs:893-904`). Without a session rmcp opens no
//!   standalone GET stream (`streamable_http_client.rs:968-978`) and sends no
//!   DELETE on close. A session would buy nothing here: there is one BIR, we
//!   never push server-to-client messages, and a session store would be one
//!   more thing to expire. A stray `Mcp-Session-Id` from another client is
//!   ignored rather than answered with 404, since we never issued one.
//! - `GET` and `DELETE` get `405`. rmcp reads 405 on GET as "server does not
//!   support SSE" and carries on (`common/reqwest/streamable_http_client.rs:94-96`,
//!   `streamable_http_client.rs:735-737`) and 405 on DELETE as "server does
//!   not support deleting sessions" (`common/reqwest/streamable_http_client.rs:163-166`).
//! - `server/discover` (rmcp's `Auto`/`Discover` lifecycles) falls through to
//!   the dispatcher's JSON-RPC method-not-found error, which rmcp classifies as
//!   a legacy server and falls back to `initialize` (`service/client.rs:801-820`).
//!
//! Guards, in order: protected-resource metadata (no token), path, `Origin`
//! (DNS-rebinding guard from the MCP spec: present and not loopback is 403),
//! bearer token (401 with `WWW-Authenticate: Bearer resource_metadata=…`),
//! method, content type (`application/json` only, which also keeps a browser's
//! no-preflight "simple" POST out), body size. The token is never logged.
//!
//! The bearer token is an OAuth access token from buwiz-server, checked by
//! [`crate::oauth::Introspection`]. It is not a shared secret.
use crate::license::{License, UNAUTHENTICATED};
use crate::oauth::{self, AdmitError, Introspection, Resource};
use crate::server::{BirHost, Server};
use serde_json::Value;
use std::io::Read;
use std::net::{IpAddr, SocketAddr};

/// The one path MCP is served on.
pub const MCP_PATH: &str = "/mcp";
/// RFC 9728 protected-resource metadata, origin form and path-appended form.
pub const METADATA_PATH: &str = "/.well-known/oauth-protected-resource";
pub const METADATA_MCP_PATH: &str = "/.well-known/oauth-protected-resource/mcp";
/// Largest request body accepted. A tool call carries a handful of boxes.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

const SUPPORTED_PROTOCOL_HEADER: &str = "mcp-protocol-version";

/// Parse a `--http` address and refuse anything that is not a loopback IP.
pub fn loopback_addr(addr: &str) -> Result<SocketAddr, String> {
    let parsed: SocketAddr = addr.parse().map_err(|_| {
        format!("--http needs an IP:PORT such as 127.0.0.1:8765 or [::1]:8765, got `{addr}`")
    })?;
    if !parsed.ip().is_loopback() {
        return Err(format!(
            "--http refuses to bind {parsed}: only loopback addresses (127.0.0.0/8, ::1) are allowed"
        ));
    }
    Ok(parsed)
}

/// Bind the listener. Checks the address is loopback first.
pub fn bind(addr: &str) -> Result<tiny_http::Server, String> {
    let addr = loopback_addr(addr)?;
    tiny_http::Server::http(addr).map_err(|error| format!("cannot listen on {addr}: {error}"))
}

/// One HTTP answer, before it is written to the socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    fn empty(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn text(status: u16, message: &str) -> Self {
        Self {
            status,
            headers: vec![("Content-Type", "text/plain; charset=utf-8".into())],
            body: message.as_bytes().to_vec(),
        }
    }

    fn json(status: u16, value: &Value) -> Self {
        Self {
            status,
            headers: vec![("Content-Type", "application/json".into())],
            body: value.to_string().into_bytes(),
        }
    }

    fn with_header(mut self, name: &'static str, value: &str) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}

/// Answer one HTTP request. `headers` are `(name, value)` pairs as received.
///
/// A failed check leaves the server's license closed, so a later tool call
/// cannot reuse a previous request's grant. Requests are handled one at a time.
pub fn respond<H: BirHost>(
    server: &mut Server<H>,
    auth: &Introspection,
    resource: &Resource,
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Reply {
    server.set_license(License::settled(Err(UNAUTHENTICATED.to_string())));
    let header = |name: &str| {
        headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    };

    let path = url.split(['?', '#']).next().unwrap_or_default();
    if path == METADATA_PATH || path == METADATA_MCP_PATH {
        if method != "GET" {
            return Reply::text(405, "method not allowed").with_header("Allow", "GET");
        }
        return Reply::json(200, &resource.document())
            .with_header("Access-Control-Allow-Origin", "*")
            .with_header("Cache-Control", "no-store");
    }
    if path != MCP_PATH {
        return Reply::text(404, "not found: MCP is served at /mcp");
    }
    if let Some(origin) = header("origin")
        && !is_loopback_origin(origin)
    {
        return Reply::text(403, "forbidden: Origin is not a loopback origin");
    }
    let presented = bearer_token(header("authorization"));
    let profiles = match presented.as_deref() {
        None => {
            return unauthorized(resource, None, None);
        }
        Some(token) => match auth.admit(token, &resource.mcp) {
            Ok(profiles) => profiles,
            Err(AdmitError::Invalid) => {
                return unauthorized(resource, Some("invalid_token"), None);
            }
            Err(AdmitError::Audience) => {
                return unauthorized(
                    resource,
                    Some("invalid_token"),
                    Some("audience does not match this resource"),
                );
            }
            Err(AdmitError::Unavailable) => {
                return Reply::text(503, "the authorization server could not be reached")
                    .with_header("Cache-Control", "no-store");
            }
        },
    };
    server.set_license(License::settled(oauth::entitlement(&profiles)));
    if method != "POST" {
        // GET: no standalone SSE stream. DELETE: no sessions. See the module note.
        return Reply::text(405, "method not allowed").with_header("Allow", "POST");
    }
    if let Some(version) = header(SUPPORTED_PROTOCOL_HEADER)
        && !crate::server::SUPPORTED_PROTOCOLS.contains(&version.trim())
    {
        return Reply::text(400, "bad request: unsupported MCP-Protocol-Version");
    }
    let is_json = header("content-type").is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
    });
    if !is_json {
        return Reply::text(415, "unsupported media type: send application/json");
    }
    if body.len() > MAX_BODY_BYTES {
        return Reply::text(413, "payload too large");
    }
    let Ok(text) = std::str::from_utf8(body) else {
        return Reply::text(400, "bad request: body is not UTF-8");
    };
    let reply = match serde_json::from_str::<Value>(text) {
        // A client's JSON-RPC response (we never send requests, so it answers
        // nothing) is accepted with 202 as the spec asks, not dispatched.
        Ok(message) if is_client_response(&message) => return Reply::empty(202),
        Ok(message) => server.handle(message),
        // Same parse-error reply as stdio.
        Err(_) => server
            .handle_line(text)
            .map(|line| serde_json::from_str(&line).expect("server emits JSON")),
    };
    match reply {
        Some(value) => {
            // A JSON-RPC error for a body that was not a usable message is a
            // bad request; anything else that came back answers a request.
            let unusable =
                value.get("id").is_some_and(Value::is_null) && value.get("error").is_some();
            Reply::json(if unusable { 400 } else { 200 }, &value)
        }
        // Notifications and blank bodies.
        None => Reply::empty(202),
    }
}

fn unauthorized(resource: &Resource, error: Option<&str>, description: Option<&str>) -> Reply {
    Reply::text(
        401,
        "unauthorized: a bearer token for this resource is required",
    )
    .with_header("WWW-Authenticate", &resource.challenge(error, description))
    .with_header("Access-Control-Allow-Origin", "*")
    .with_header("Cache-Control", "no-store")
}

/// Serve until the listener is unblocked or fails. Requests are handled one
/// at a time: the host drives one BIR form anyway.
pub fn serve_http<H: BirHost>(
    server: &mut Server<H>,
    listener: &tiny_http::Server,
    auth: &Introspection,
    resource: &Resource,
) {
    for mut request in listener.incoming_requests() {
        let method = request.method().as_str().to_ascii_uppercase();
        let url = request.url().to_string();
        let headers: Vec<(String, String)> = request
            .headers()
            .iter()
            .map(|h| {
                (
                    h.field.as_str().as_str().to_string(),
                    h.value.as_str().to_string(),
                )
            })
            .collect();
        let mut body = Vec::new();
        let read = request
            .as_reader()
            .take(MAX_BODY_BYTES as u64 + 1)
            .read_to_end(&mut body);
        let reply = match read {
            Ok(_) => respond(server, auth, resource, &method, &url, &headers, &body),
            Err(_) => Reply::text(400, "bad request: body could not be read"),
        };
        let mut response =
            tiny_http::Response::from_data(reply.body).with_status_code(reply.status);
        for (name, value) in &reply.headers {
            if let Ok(header) = tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()) {
                response.add_header(header);
            }
        }
        // A client that hung up mid-reply is its own problem.
        let _ = request.respond(response);
    }
}

fn is_client_response(message: &Value) -> bool {
    message.as_object().is_some_and(|object| {
        !object.contains_key("method")
            && (object.contains_key("result") || object.contains_key("error"))
    })
}

/// The token of an `Authorization: Bearer <token>` value.
fn bearer_token(header: Option<&str>) -> Option<String> {
    let value = header?.trim();
    let (scheme, provided) = value.split_once(' ')?;
    let provided = provided.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !provided.is_empty()).then(|| provided.to_string())
}

/// Time depends only on `expected`'s length, never on where a guess differs.
#[cfg(test)]
fn constant_time_eq(provided: &[u8], expected: &[u8]) -> bool {
    let mut diff = provided.len() ^ expected.len();
    for (i, byte) in expected.iter().enumerate() {
        diff |= usize::from(byte ^ provided.get(i).copied().unwrap_or(0));
    }
    diff == 0
}

/// `http(s)://` + a loopback host (`localhost`, 127.0.0.0/8, `[::1]`) +
/// optional port, and nothing else. `null` and every other origin are foreign.
fn is_loopback_origin(origin: &str) -> bool {
    let origin = origin.trim();
    let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    let (host, port) = if let Some(bracketed) = rest.strip_prefix('[') {
        let Some((host, after)) = bracketed.split_once(']') else {
            return false;
        };
        (host, after)
    } else {
        match rest.find(':') {
            Some(at) => (&rest[..at], &rest[at..]),
            None => (rest, ""),
        }
    };
    let port_ok = port.is_empty()
        || port
            .strip_prefix(':')
            .is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    let host_ok = host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
    port_ok && host_ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::{self, Introspection};
    use crate::server::AppRecord;
    use crate::tools::HostCall;
    use serde_json::json;
    use std::io::Write;
    use std::net::TcpStream;
    use std::sync::Arc;

    const TOKEN: &str = "test-token-not-a-secret";
    const OTHER_AUDIENCE: &str = "other-audience-token";
    const EMPTY_PASS: &str = "empty-pass-token";

    /// Stands in for BIR. Never reaches a real app; no app is running.
    struct MockHost;

    impl BirHost for MockHost {
        fn apps(&mut self) -> Result<Vec<AppRecord>, String> {
            Ok(Vec::new())
        }

        fn resolve(&mut self, id: &str) -> Result<AppRecord, String> {
            Err(format!("no AgentHost is running for '{id}'"))
        }

        fn alive(&mut self, _app: &AppRecord) -> bool {
            false
        }

        fn call(
            &mut self,
            _app: &AppRecord,
            call: HostCall,
            _args: Value,
        ) -> Result<Value, String> {
            Ok(json!({"mock": format!("{call:?}")}))
        }
    }

    /// A listener on an ephemeral loopback port, served on a thread.
    /// The authorization server is a stub on another loopback port.
    struct Running {
        listener: Arc<tiny_http::Server>,
        auth_listener: Arc<tiny_http::Server>,
        addr: SocketAddr,
        resource: oauth::Resource,
        thread: Option<std::thread::JoinHandle<()>>,
        auth_thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Running {
        fn start() -> Self {
            let listener = Arc::new(bind("127.0.0.1:0").unwrap());
            let addr = listener.server_addr().to_ip().unwrap();
            let auth_listener = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
            let auth_addr = auth_listener.server_addr().to_ip().unwrap();
            let resource = oauth::Resource::from_bind(addr, &format!("http://{auth_addr}"));
            let expected_audience = resource.mcp.clone();
            let serving_auth = Arc::clone(&auth_listener);
            let auth_thread = std::thread::spawn(move || {
                for mut request in serving_auth.incoming_requests() {
                    let mut body = String::new();
                    let _ = request.as_reader().read_to_string(&mut body);
                    let token = form_value(&body, "token");
                    let reply = if token.as_deref() == Some(TOKEN)
                        || token.as_deref() == Some(EMPTY_PASS)
                    {
                        let permissions = if token.as_deref() == Some(EMPTY_PASS) {
                            json!([])
                        } else {
                            json!(["profile.view"])
                        };
                        json!({
                            "active": true,
                            "aud": expected_audience,
                            "profiles": [{"profile_id": "p1", "permissions": permissions}]
                        })
                        .to_string()
                    } else if token.as_deref() == Some(OTHER_AUDIENCE) {
                        json!({
                            "active": true,
                            "aud": "http://127.0.0.1:9/mcp",
                            "profiles": [{"profile_id": "p1", "permissions": ["profile.view"]}]
                        })
                        .to_string()
                    } else {
                        json!({"active": false}).to_string()
                    };
                    let _ = request
                        .respond(tiny_http::Response::from_string(reply).with_status_code(200));
                }
            });
            let serving = Arc::clone(&listener);
            let auth = Introspection::new(resource.authorization_server.clone());
            let bound = resource.clone();
            let thread = std::thread::spawn(move || {
                let mut server = Server::new(MockHost);
                serve_http(&mut server, &serving, &auth, &bound);
            });
            Self {
                listener,
                auth_listener,
                addr,
                resource,
                thread: Some(thread),
                auth_thread: Some(auth_thread),
            }
        }

        /// Raw HTTP/1.1 over a fresh connection; returns (status, headers, body).
        fn send(
            &self,
            method: &str,
            path: &str,
            headers: &[(&str, &str)],
            body: &str,
        ) -> (u16, String, String) {
            let mut stream = TcpStream::connect(self.addr).unwrap();
            let mut request = format!(
                "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n",
                self.addr,
                body.len()
            );
            for (name, value) in headers {
                request.push_str(&format!("{name}: {value}\r\n"));
            }
            request.push_str("\r\n");
            request.push_str(body);
            stream.write_all(request.as_bytes()).unwrap();
            let mut raw = String::new();
            stream.read_to_string(&mut raw).unwrap();
            let (head, body) = raw.split_once("\r\n\r\n").unwrap();
            let status = head.split(' ').nth(1).unwrap().parse().unwrap();
            (status, head.to_string(), body.to_string())
        }

        fn post(&self, message: &Value, extra: &[(&str, &str)]) -> (u16, String, String) {
            let bearer = format!("Bearer {TOKEN}");
            let mut headers = vec![
                ("Authorization", bearer.as_str()),
                ("Content-Type", "application/json"),
                ("Accept", "application/json, text/event-stream"),
            ];
            headers.extend_from_slice(extra);
            self.send("POST", MCP_PATH, &headers, &message.to_string())
        }
    }

    fn form_value(body: &str, key: &str) -> Option<String> {
        for pair in body.split('&') {
            let (name, value) = pair.split_once('=')?;
            if name == key {
                return Some(value.replace('+', " "));
            }
        }
        None
    }

    impl Drop for Running {
        fn drop(&mut self) {
            self.listener.unblock();
            self.auth_listener.unblock();
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
            if let Some(thread) = self.auth_thread.take() {
                thread.join().unwrap();
            }
        }
    }

    fn initialize() -> Value {
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "0"}
        }})
    }

    fn tool_names(list: &Value) -> Vec<String> {
        list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn initialize_then_tools_list_matches_stdio_and_has_no_filing_tool() {
        let running = Running::start();
        let (status, head, body) = running.post(&initialize(), &[]);
        assert_eq!(status, 200, "{head}\n{body}");
        assert!(
            head.to_ascii_lowercase()
                .contains("content-type: application/json"),
            "{head}"
        );
        let init: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "bir-mcp");
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");

        let (status, _, body) = running.post(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            &[("MCP-Protocol-Version", "2025-06-18")],
        );
        assert_eq!((status, body.as_str()), (202, ""));

        let list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
        let (status, _, body) = running.post(&list, &[("MCP-Protocol-Version", "2025-06-18")]);
        assert_eq!(status, 200);
        let over_http: Value = serde_json::from_str(&body).unwrap();

        // Same dispatch as stdio, so the same tool set.
        let mut stdio = Server::new(MockHost);
        let over_stdio = stdio.handle(list).unwrap();
        assert_eq!(over_http, over_stdio);
        let names = tool_names(&over_http);
        assert_eq!(names.len(), crate::tools::TOOLS.len());
        assert!(
            names.iter().any(|name| name == "bir_form_send"),
            "stdio and --http both list bir_form_send: {names:?}"
        );
        for name in &names {
            for banned in ["submit", "queue", "file", "pay"] {
                // `bir_form_*` contains "form", not "file"; check whole words.
                assert!(
                    !name.split('_').any(|part| part == banned),
                    "{name} looks like a filing tool"
                );
            }
        }

        let call = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": {"name": "bir_status", "arguments": {}}});
        let (status, _, body) = running.post(&call, &[]);
        assert_eq!(status, 200);
        let reply: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(reply["result"]["isError"], false, "{reply}");
        assert!(
            reply["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("\"connected\": false"),
            "{reply}"
        );

        // Over HTTP too, form tools refuse until an app is selected.
        let open = json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call",
                          "params": {"name": "bir_profiles_list", "arguments": {}}});
        let (status, _, body) = running.post(&open, &[]);
        assert_eq!(status, 200);
        let reply: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(reply["result"]["isError"], true, "{reply}");
        assert_eq!(
            reply["result"]["content"][0]["text"],
            crate::server::NOT_SELECTED
        );

        let send = json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call",
                          "params": {"name": "bir_form_send", "arguments": {}}});
        let (status, _, body) = running.post(&send, &[]);
        assert_eq!(status, 200);
        let reply: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(reply["result"]["isError"], true, "{reply}");
        assert_eq!(
            reply["result"]["content"][0]["text"],
            crate::server::NOT_SELECTED,
            "bir_form_send is a real tool; it waits for a selected app, then form.send"
        );

        let submit = json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
                            "params": {"name": "bir_form_submit", "arguments": {}}});
        let (status, _, body) = running.post(&submit, &[]);
        assert_eq!(status, 200);
        let reply: Value = serde_json::from_str(&body).unwrap();
        assert!(
            reply["error"]["message"]
                .as_str()
                .unwrap()
                .contains("unknown tool")
        );
    }

    #[test]
    fn missing_or_wrong_bearer_is_401() {
        let running = Running::start();
        let body = initialize().to_string();
        let json = ("Content-Type", "application/json");
        for auth in [
            None,
            Some("Bearer wrong"),
            Some("Bearer test-token-not-a-secre"),
            Some("Bearer test-token-not-a-secretX"),
            Some("Basic test-token-not-a-secret"),
            Some("test-token-not-a-secret"),
            Some("Bearer "),
        ] {
            let mut headers = vec![json];
            if let Some(auth) = auth {
                headers.push(("Authorization", auth));
            }
            let (status, head, body) = running.send("POST", MCP_PATH, &headers, &body);
            assert_eq!(status, 401, "{auth:?}");
            assert!(
                head.contains(&format!(
                    "resource_metadata=\"{}\"",
                    running.resource.metadata
                )),
                "{head}"
            );
            assert!(!body.contains(TOKEN));
            assert!(!head.contains(TOKEN));
        }
        // Scheme is case-insensitive.
        let lower = format!("bearer {TOKEN}");
        let (status, _, _) =
            running.send("POST", MCP_PATH, &[json, ("Authorization", &lower)], &body);
        assert_eq!(status, 200);

        let other = format!("Bearer {OTHER_AUDIENCE}");
        let (status, head, _) = running.send(
            "POST",
            MCP_PATH,
            &[json, ("Authorization", other.as_str())],
            &body,
        );
        assert_eq!(status, 401);
        assert!(
            head.contains("audience does not match this resource"),
            "{head}"
        );

        let (status, head, body) = running.send("GET", METADATA_MCP_PATH, &[], "");
        assert_eq!(status, 200, "{head}");
        assert!(
            head.to_ascii_lowercase()
                .contains("access-control-allow-origin: *"),
            "{head}"
        );
        let document: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(document["resource"], running.resource.mcp);
        assert_eq!(
            document["authorization_servers"][0],
            running.resource.authorization_server
        );
        let (status, _, body) = running.send("GET", METADATA_PATH, &[], "");
        assert_eq!(status, 200);
        let origin_document: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(origin_document, document);
    }

    #[test]
    fn a_token_with_no_working_pass_refuses_the_tool() {
        let running = Running::start();
        let bearer = format!("Bearer {EMPTY_PASS}");
        let call = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": {"name": "bir_status", "arguments": {}}});
        let (status, _, body) = running.send(
            "POST",
            MCP_PATH,
            &[
                ("Authorization", bearer.as_str()),
                ("Content-Type", "application/json"),
            ],
            &call.to_string(),
        );
        assert_eq!(status, 200, "{body}");
        let reply: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(reply["result"]["isError"], true, "{reply}");
        assert_eq!(
            reply["result"]["content"][0]["text"],
            crate::license::UNLICENSED
        );
    }

    #[test]
    fn foreign_origin_is_403_and_loopback_origin_is_allowed() {
        let running = Running::start();
        for origin in [
            "http://evil.example",
            "http://127.0.0.1.evil.example",
            "https://localhost.evil.example:443",
            "http://10.0.0.5:8765",
            "null",
            "file://",
        ] {
            let (status, _, _) = running.post(&initialize(), &[("Origin", origin)]);
            assert_eq!(status, 403, "{origin}");
        }
        for origin in [
            "http://127.0.0.1:8765",
            "http://localhost",
            "https://localhost:3000",
            "http://[::1]:8765",
        ] {
            let (status, _, _) = running.post(&initialize(), &[("Origin", origin)]);
            assert_eq!(status, 200, "{origin}");
        }
    }

    #[test]
    fn notification_is_202_with_empty_body() {
        let running = Running::start();
        let (status, head, body) = running.post(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            &[],
        );
        assert_eq!(status, 202, "{head}");
        assert!(body.is_empty());
        // A client response to a request we never sent is also just accepted.
        let (status, _, body) =
            running.post(&json!({"jsonrpc": "2.0", "id": 9, "result": {}}), &[]);
        assert_eq!((status, body.as_str()), (202, ""));
    }

    #[test]
    fn get_delete_wrong_path_type_and_version() {
        let running = Running::start();
        let bearer = format!("Bearer {TOKEN}");
        let auth = ("Authorization", bearer.as_str());
        let (status, head, _) = running.send(
            "GET",
            MCP_PATH,
            &[auth, ("Accept", "text/event-stream")],
            "",
        );
        assert_eq!(status, 405);
        assert!(head.contains("Allow: POST"), "{head}");
        let (status, _, _) = running.send("DELETE", MCP_PATH, &[auth], "");
        assert_eq!(status, 405);
        let (status, _, _) = running.send("POST", "/other", &[auth], "{}");
        assert_eq!(status, 404);
        let (status, _, _) = running.send(
            "POST",
            MCP_PATH,
            &[auth, ("Content-Type", "text/plain")],
            &initialize().to_string(),
        );
        assert_eq!(status, 415);
        let (status, _, _) = running.post(&initialize(), &[("MCP-Protocol-Version", "1999-01-01")]);
        assert_eq!(status, 400);
        let (status, _, body) = running.send(
            "POST",
            MCP_PATH,
            &[auth, ("Content-Type", "application/json")],
            "{nope",
        );
        assert_eq!(status, 400);
        let parse: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parse["error"]["code"], -32700);
    }

    #[test]
    fn non_loopback_bind_is_refused() {
        for addr in [
            "0.0.0.0:8765",
            "192.168.1.10:8765",
            "[::]:8765",
            "8.8.8.8:80",
        ] {
            let error = loopback_addr(addr).unwrap_err();
            assert!(error.contains("only loopback"), "{addr}: {error}");
            assert!(bind(addr).is_err());
        }
        assert!(
            loopback_addr("localhost:8765")
                .unwrap_err()
                .contains("IP:PORT")
        );
        assert!(loopback_addr("127.0.0.1").is_err());
        assert_eq!(
            loopback_addr("127.0.0.1:8765").unwrap(),
            "127.0.0.1:8765".parse().unwrap()
        );
        assert!(loopback_addr("[::1]:8765").is_ok());
    }

    #[test]
    fn constant_time_compare_and_origin_parser() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"ab", b"abc"));
        assert!(!constant_time_eq(b"abcd", b"abc"));
        assert!(!constant_time_eq(b"", b"abc"));
        assert!(is_loopback_origin("http://127.0.0.2:1"));
        assert!(!is_loopback_origin("http://127.0.0.1:"));
        assert!(!is_loopback_origin("http://127.0.0.1:80/path"));
        assert!(!is_loopback_origin("http://[::1"));
    }
}
