//! Buwiz sign-in gate for every bir-mcp tool.
//!
//! Left open unless `BUWIZ_MCP_URL` is set. When it is set, each tool asks
//! buwiz-server's MCP endpoint whether `BUWIZ_ACCESS_TOKEN` is a live agent
//! token and whether that agent can see at least one taxpayer through a
//! working pass. buwiz-server already answers 401 for a bad token and lists
//! a profile with empty `permissions` when the pass no longer covers it
//! (`list_tax_profiles`). This module turns those two answers into one
//! refusal the model can show the user, and it does that before bir-mcp
//! talks to a BIR app.
//!
//! There is no separate subscription product on buwiz-server: the working
//! pass is the license. A signed-in agent with no usable pass is refused
//! here the same way an unknown token is, with a different sentence.

use serde_json::{Value, json};
use std::time::Duration;

/// Shown when the token is missing or buwiz-server answers 401.
pub const UNAUTHENTICATED: &str = "Buwiz sign-in required. Set BUWIZ_ACCESS_TOKEN to a live \
buwiz-server agent token. bir-mcp will not drive BIR until that token is accepted.";

/// Shown when the token is live but no connected taxpayer has a working pass.
pub const UNLICENSED: &str = "Buwiz access required. This token is signed in, but no connected \
taxpayer has a working pass. bir-mcp will not open or fill a return until that access is in place.";

const LIST_PROFILES: &str = "list_tax_profiles";
const PROTOCOL: &str = "2025-06-18";

/// Whether tools may run.
#[derive(Debug, Clone)]
pub enum License {
    /// No `BUWIZ_MCP_URL`. Tools run without a buwiz-server round trip.
    Open,
    /// Every tool checks this origin with this agent token first.
    Required { base: String, token: String },
}

impl License {
    pub fn open() -> Self {
        Self::Open
    }

    /// `base` is the buwiz-server origin, with or without a trailing slash.
    pub fn required(base: impl Into<String>, token: impl Into<String>) -> Self {
        let base = base.into().trim().trim_end_matches('/').to_string();
        Self::Required {
            base,
            token: token.into(),
        }
    }

    /// `url` is `BUWIZ_MCP_URL`, `token` is `BUWIZ_ACCESS_TOKEN`. An empty URL
    /// leaves the gate open. A URL with no token refuses to start.
    pub fn from_parts(url: Option<&str>, token: Option<&str>) -> Result<Self, String> {
        let Some(url) = url.map(str::trim).filter(|url| !url.is_empty()) else {
            return Ok(Self::Open);
        };
        let token = token.unwrap_or("").trim();
        if token.is_empty() {
            return Err(UNAUTHENTICATED.to_string());
        }
        Ok(Self::required(url, token.to_string()))
    }

    /// `Ok` when this tool may run. The error string is the tool result text.
    pub fn allow(&self) -> Result<(), String> {
        match self {
            Self::Open => Ok(()),
            Self::Required { base, token } => check(base, token),
        }
    }
}

fn check(base: &str, token: &str) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Err(UNAUTHENTICATED.to_string());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("could not prepare the Buwiz license check: {error}"))?;
    let response = client
        .post(format!("{base}/mcp"))
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", PROTOCOL)
        .body(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": { "name": LIST_PROFILES, "arguments": {} }
            })
            .to_string(),
        )
        .send()
        .map_err(|error| {
            format!("could not reach buwiz-server at {base} to check the Buwiz pass: {error}")
        })?;
    let status = response.status().as_u16();
    let body = response.text().map_err(|error| {
        format!("buwiz-server closed the license check before a body arrived: {error}")
    })?;
    interpret(status, &body)
}

/// Map a buwiz-server `/mcp` reply to allow or refuse.
pub fn interpret(status: u16, body: &str) -> Result<(), String> {
    if status == 401 {
        return Err(UNAUTHENTICATED.to_string());
    }
    if status != 200 {
        return Err(format!(
            "buwiz-server answered HTTP {status} while checking the Buwiz pass"
        ));
    }
    let value: Value = serde_json::from_str(body)
        .map_err(|_| "buwiz-server returned a license reply bir-mcp could not read".to_string())?;
    if value.get("error").is_some() {
        return Err(UNAUTHENTICATED.to_string());
    }
    let Some(result) = value.get("result") else {
        return Err("buwiz-server license reply had no result".to_string());
    };
    if result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let text = result
            .pointer("/content/0/text")
            .and_then(Value::as_str)
            .unwrap_or("");
        if text.contains("Not allowed") || text.to_ascii_lowercase().contains("working pass") {
            return Err(UNLICENSED.to_string());
        }
        return Err(format!("buwiz-server refused the license check: {text}"));
    }
    let data = result.get("structuredContent").cloned().or_else(|| {
        result
            .pointer("/content/0/text")
            .and_then(Value::as_str)
            .and_then(|text| serde_json::from_str(text).ok())
    });
    let Some(profiles) = data
        .as_ref()
        .and_then(|data| data.get("profiles"))
        .and_then(Value::as_array)
    else {
        return Err("buwiz-server license reply had no profile list".to_string());
    };
    let licensed = profiles.iter().any(|profile| {
        profile
            .get("permissions")
            .and_then(Value::as_array)
            .is_some_and(|ids| !ids.is_empty())
    });
    if licensed {
        Ok(())
    } else {
        Err(UNLICENSED.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn licensed_body() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "isError": false,
                "structuredContent": {
                    "profiles": [{
                        "profile_id": "p1",
                        "permissions": ["profile.view"]
                    }]
                }
            }
        })
    }

    #[test]
    fn missing_url_stays_open_and_a_url_needs_a_token() {
        assert!(matches!(
            License::from_parts(None, None).unwrap(),
            License::Open
        ));
        assert!(matches!(
            License::from_parts(Some("  "), Some("token")).unwrap(),
            License::Open
        ));
        assert_eq!(
            License::from_parts(Some("http://127.0.0.1:9"), None).unwrap_err(),
            UNAUTHENTICATED
        );
        assert_eq!(
            License::from_parts(Some("http://127.0.0.1:9/"), Some("  ")).unwrap_err(),
            UNAUTHENTICATED
        );
        match License::from_parts(Some("http://127.0.0.1:9/"), Some(" secret ")).unwrap() {
            License::Required { base, token } => {
                assert_eq!(base, "http://127.0.0.1:9");
                assert_eq!(token, "secret");
            }
            License::Open => panic!("expected a required license"),
        }
    }

    #[test]
    fn replies_distinguish_signed_out_unlicensed_and_licensed() {
        assert_eq!(
            interpret(401, r#"{"error":"invalid_token"}"#).unwrap_err(),
            UNAUTHENTICATED
        );
        assert_eq!(
            interpret(500, "nope").unwrap_err(),
            "buwiz-server answered HTTP 500 while checking the Buwiz pass"
        );
        let empty = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "isError": false,
                "structuredContent": {
                    "profiles": [{
                        "profile_id": "p1",
                        "name": "(no access right now)",
                        "permissions": []
                    }]
                }
            }
        });
        assert_eq!(interpret(200, &empty.to_string()).unwrap_err(), UNLICENSED);
        let none = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "isError": false,
                "content": [{ "type": "text", "text": "{\"profiles\":[]}" }]
            }
        });
        assert_eq!(interpret(200, &none.to_string()).unwrap_err(), UNLICENSED);
        assert!(interpret(200, &licensed_body().to_string()).is_ok());
        let refused = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "isError": true,
                "content": [{
                    "type": "text",
                    "text": "Not allowed: this profile is not connected to you, or the taxpayer's access no longer covers it."
                }]
            }
        });
        assert_eq!(
            interpret(200, &refused.to_string()).unwrap_err(),
            UNLICENSED
        );
    }

    /// One POST to `/mcp`, then the status and body this stub was given.
    fn ask(status: u16, body: &str) -> Result<(), String> {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_ip().unwrap();
        let body = body.to_string();
        let served = std::thread::spawn(move || {
            let mut request = server.recv().unwrap();
            assert_eq!(request.method().as_str(), "POST");
            assert_eq!(request.url(), "/mcp");
            let authorization = request
                .headers()
                .iter()
                .find(|header| header.field.equiv("Authorization"))
                .map(|header| header.value.as_str().to_string())
                .unwrap_or_default();
            let mut incoming = String::new();
            std::io::Read::read_to_string(request.as_reader(), &mut incoming).unwrap();
            assert!(incoming.contains(LIST_PROFILES), "{incoming}");
            let response = tiny_http::Response::from_string(body)
                .with_status_code(status)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                        .unwrap(),
                );
            request.respond(response).unwrap();
            authorization
        });
        let verdict = License::required(format!("http://{addr}"), "agent-token").allow();
        assert_eq!(served.join().unwrap(), "Bearer agent-token");
        verdict
    }

    #[test]
    fn a_live_stub_refuses_401_and_an_empty_pass_and_allows_a_working_pass() {
        assert_eq!(
            ask(401, r#"{"error":"invalid_token"}"#).unwrap_err(),
            UNAUTHENTICATED
        );
        let empty = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "isError": false,
                "structuredContent": { "profiles": [] }
            }
        });
        assert_eq!(ask(200, &empty.to_string()).unwrap_err(), UNLICENSED);
        assert!(ask(200, &licensed_body().to_string()).is_ok());
        assert_eq!(
            License::required("http://127.0.0.1:9", "  ")
                .allow()
                .unwrap_err(),
            UNAUTHENTICATED
        );
    }
}
