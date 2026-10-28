//! `bir-mcp --http` as an OAuth 2.1 protected resource.
//!
//! The MCP authorization spec (2025-06-18) makes the HTTP server a resource
//! server. This module publishes the protected-resource metadata, builds the
//! `WWW-Authenticate` challenge, and checks bearer tokens with buwiz-server's
//! introspection endpoint. Stdio keeps the desktop account gate and does not
//! use this module.
//!
//! The authorization server is the account origin the desktop app already
//! resolves: `https://account.buwiz.com` in a release build,
//! `http://localhost:3008` in a debug build, with `BUWIZ_ACCOUNT_URL` and
//! `BUWIZ_SERVER_URL` overriding that the same way (`option_env!`, then the
//! process environment). A token whose audience is any other resource is
//! refused.

use crate::license::{UNLICENSED, profiles_have_access};
use bir_core::cloud_account::{self, BuildProfile, ResolvedServer, ServerUrlError, UrlSource};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::time::Duration;

/// Scope buwiz-server puts on agent tokens. Not a secret.
pub const SCOPE: &str = "tax_profiles";

const METADATA_SUFFIX: &str = "/.well-known/oauth-protected-resource/mcp";

/// Why a bearer token may not call this resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmitError {
    /// Missing, unknown, expired, or revoked.
    Invalid,
    /// The token was minted for a different resource.
    Audience,
    /// The authorization server could not be asked.
    Unavailable,
}

/// One loopback MCP resource and the authorization server that issues for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    /// Canonical MCP URL, also the OAuth audience. `http://127.0.0.1:port/mcp`.
    pub mcp: String,
    /// RFC 9728 path-appended metadata URL.
    pub metadata: String,
    /// Issuer. `https://account.buwiz.com` or `http://localhost:3008`.
    pub authorization_server: String,
}

impl Resource {
    pub fn from_bind(bound: SocketAddr, authorization_server: &str) -> Self {
        let origin = format!("http://{bound}");
        Self {
            mcp: format!("{origin}/mcp"),
            metadata: format!("{origin}{METADATA_SUFFIX}"),
            authorization_server: authorization_server
                .trim()
                .trim_end_matches('/')
                .to_string(),
        }
    }

    pub fn document(&self) -> Value {
        json!({
            "resource": self.mcp,
            "authorization_servers": [self.authorization_server],
            "bearer_methods_supported": ["header"],
            "scopes_supported": [SCOPE],
            "resource_name": "Buwiz BIR drafts",
        })
    }

    /// `WWW-Authenticate` challenge. `error` is an RFC 6750 error code.
    pub fn challenge(&self, error: Option<&str>, description: Option<&str>) -> String {
        let mut challenge = format!(
            "Bearer resource_metadata=\"{}\", scope=\"{SCOPE}\"",
            self.metadata
        );
        if let Some(error) = error {
            challenge.push_str(&format!(", error=\"{error}\""));
        }
        if let Some(description) = description {
            challenge.push_str(&format!(
                ", error_description=\"{}\"",
                description.replace('"', "'")
            ));
        }
        challenge
    }
}

/// Account origin when one was configured, otherwise an explicitly configured
/// API origin, otherwise the account-origin default for this build.
pub fn resolve_authorization_server(
    account_runtime: Option<&str>,
    account_compiled: Option<&str>,
    server_runtime: Option<&str>,
    server_compiled: Option<&str>,
    profile: BuildProfile,
) -> Result<ResolvedServer, ServerUrlError> {
    let account = cloud_account::resolve_account_url(account_runtime, account_compiled, profile)?;
    if account.source != UrlSource::Default {
        return Ok(account);
    }
    let server = cloud_account::resolve_server_url(server_runtime, server_compiled, profile)?;
    if server.source != UrlSource::Default {
        return Ok(server);
    }
    Ok(account)
}

/// The authorization server for this process.
pub fn configured_authorization_server() -> Result<String, String> {
    let account_runtime = std::env::var(cloud_account::ENV_ACCOUNT_URL).ok();
    let server_runtime = std::env::var(cloud_account::ENV_SERVER_URL).ok();
    resolve_authorization_server(
        account_runtime.as_deref(),
        option_env!("BUWIZ_ACCOUNT_URL"),
        server_runtime.as_deref(),
        option_env!("BUWIZ_SERVER_URL"),
        cloud_account::active_build_profile(),
    )
    .map(|resolved| resolved.url)
    .map_err(|error| error.to_string())
}

/// Checks a bearer token against buwiz-server introspection.
pub struct Introspection {
    issuer: String,
}

impl Introspection {
    pub fn new(issuer: impl Into<String>) -> Self {
        Self {
            issuer: issuer.into().trim().trim_end_matches('/').to_string(),
        }
    }

    pub fn admit(&self, token: &str, resource: &str) -> Result<Vec<Value>, AdmitError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(AdmitError::Invalid);
        }
        let client = bir_core::loopback::localhost_ipv4_first(reqwest::blocking::Client::builder())
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| AdmitError::Unavailable)?;
        let response = client
            .post(format!("{}/oauth/introspect", self.issuer))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("accept", "application/json")
            .body(format!(
                "token={}&token_type_hint=access_token",
                form_encode(token)
            ))
            .send()
            .map_err(|_| AdmitError::Unavailable)?;
        let status = response.status().as_u16();
        let body = response.text().map_err(|_| AdmitError::Unavailable)?;
        interpret_introspection(resource, status, &body)
    }
}

/// Map an introspection reply to the profiles the token may see.
/// The caller turns an empty permission set into [`UNLICENSED`].
pub fn interpret_introspection(
    resource: &str,
    status: u16,
    body: &str,
) -> Result<Vec<Value>, AdmitError> {
    if status != 200 {
        return Err(AdmitError::Unavailable);
    }
    let value: Value = serde_json::from_str(body).map_err(|_| AdmitError::Unavailable)?;
    if value.get("active").and_then(Value::as_bool) != Some(true) {
        return Err(AdmitError::Invalid);
    }
    if !audience_allows(resource, &value) {
        return Err(AdmitError::Audience);
    }
    Ok(value
        .get("profiles")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// Tool gate for a token that already passed the audience check.
pub fn entitlement(profiles: &[Value]) -> Result<(), String> {
    if profiles_have_access(profiles) {
        Ok(())
    } else {
        Err(UNLICENSED.to_string())
    }
}

fn audience_allows(resource: &str, value: &Value) -> bool {
    match value.get("aud") {
        Some(Value::String(audience)) => audiences_match(resource, audience),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .any(|audience| audiences_match(resource, audience)),
        _ => false,
    }
}

/// Exact audience, or the same loopback port with another loopback host spelling.
pub fn audiences_match(resource: &str, audience: &str) -> bool {
    if resource == audience {
        return true;
    }
    match (loopback_mcp(resource), loopback_mcp(audience)) {
        (Some(left), Some(right)) => left.port == right.port,
        _ => false,
    }
}

struct LoopbackMcp {
    port: u16,
}

fn loopback_mcp(uri: &str) -> Option<LoopbackMcp> {
    let rest = uri.strip_prefix("http://")?;
    if uri.contains(['?', '#', ' ', '\\']) {
        return None;
    }
    let (authority, path) = rest.split_once('/')?;
    if path != "mcp" || authority.contains('@') {
        return None;
    }
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (host, after) = inner.split_once(']')?;
        let port = after.strip_prefix(':')?;
        (host, port)
    } else {
        authority.split_once(':')?
    };
    let port = port.parse::<u16>().ok()?;
    let host = host.to_ascii_lowercase();
    if port == 0 || !matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1") {
        return None;
    }
    Some(LoopbackMcp { port })
}

fn form_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::cloud_account::{DEV_SERVER_URL, PRODUCTION_ACCOUNT_URL};

    #[test]
    fn authorization_server_follows_the_desktop_origins() {
        let release =
            resolve_authorization_server(None, None, None, None, BuildProfile::Release).unwrap();
        assert_eq!(release.url, PRODUCTION_ACCOUNT_URL);
        assert_eq!(release.source, UrlSource::Default);
        let debug =
            resolve_authorization_server(None, None, None, None, BuildProfile::Debug).unwrap();
        assert_eq!(debug.url, DEV_SERVER_URL);

        let account = resolve_authorization_server(
            Some("http://127.0.0.1:3008"),
            None,
            Some("http://127.0.0.1:9"),
            None,
            BuildProfile::Release,
        )
        .unwrap();
        assert_eq!(account.url, "http://127.0.0.1:3008");
        assert_eq!(account.source, UrlSource::Environment);

        let server_only = resolve_authorization_server(
            None,
            None,
            Some("http://127.0.0.1:3010"),
            None,
            BuildProfile::Release,
        )
        .unwrap();
        assert_eq!(server_only.url, "http://127.0.0.1:3010");
        assert_eq!(server_only.source, UrlSource::Environment);

        let compiled = resolve_authorization_server(
            None,
            Some("https://account.example"),
            Some("http://127.0.0.1:3010"),
            None,
            BuildProfile::Release,
        )
        .unwrap();
        assert_eq!(compiled.url, "https://account.example");
        assert_eq!(compiled.source, UrlSource::Build);
    }

    #[test]
    fn metadata_names_the_bind_address_and_the_authorization_server() {
        let resource =
            Resource::from_bind("127.0.0.1:8765".parse().unwrap(), "http://localhost:3008/");
        assert_eq!(resource.mcp, "http://127.0.0.1:8765/mcp");
        assert_eq!(
            resource.metadata,
            "http://127.0.0.1:8765/.well-known/oauth-protected-resource/mcp"
        );
        assert_eq!(resource.authorization_server, "http://localhost:3008");
        let document = resource.document();
        assert_eq!(document["resource"], resource.mcp);
        assert_eq!(
            document["authorization_servers"][0],
            "http://localhost:3008"
        );
        assert_eq!(document["bearer_methods_supported"][0], "header");
        assert_eq!(document["scopes_supported"][0], SCOPE);
        let challenge = resource.challenge(Some("invalid_token"), Some("audience does not match"));
        assert!(challenge.contains(&format!("resource_metadata=\"{}\"", resource.metadata)));
        assert!(challenge.contains("error=\"invalid_token\""));
        assert!(challenge.contains("scope=\"tax_profiles\""));
        let v6 = Resource::from_bind("[::1]:8765".parse().unwrap(), "http://localhost:3008");
        assert_eq!(v6.mcp, "http://[::1]:8765/mcp");
    }

    #[test]
    fn introspection_refuses_another_audience_and_an_inactive_token() {
        let resource = "http://127.0.0.1:8765/mcp";
        let licensed = json!({
            "active": true,
            "aud": resource,
            "profiles": [{"profile_id": "p1", "permissions": ["profile.view"]}]
        });
        let profiles = interpret_introspection(resource, 200, &licensed.to_string()).unwrap();
        assert!(entitlement(&profiles).is_ok());

        let alias = json!({
            "active": true,
            "aud": "http://localhost:8765/mcp",
            "profiles": [{"profile_id": "p1", "permissions": ["profile.view"]}]
        });
        assert!(interpret_introspection(resource, 200, &alias.to_string()).is_ok());

        let other = json!({
            "active": true,
            "aud": "http://localhost:3008/mcp",
            "profiles": [{"profile_id": "p1", "permissions": ["profile.view"]}]
        });
        assert_eq!(
            interpret_introspection(resource, 200, &other.to_string()).unwrap_err(),
            AdmitError::Audience
        );
        let several = json!({
            "active": true,
            "aud": ["http://localhost:3008/mcp", resource],
            "profiles": [{"permissions": ["profile.view"]}]
        });
        assert!(interpret_introspection(resource, 200, &several.to_string()).is_ok());
        assert_eq!(
            interpret_introspection(resource, 200, r#"{"active":false}"#).unwrap_err(),
            AdmitError::Invalid
        );
        assert_eq!(
            interpret_introspection(resource, 200, r#"{"active":true}"#).unwrap_err(),
            AdmitError::Audience
        );
        assert_eq!(
            interpret_introspection(resource, 503, "down").unwrap_err(),
            AdmitError::Unavailable
        );
        let empty = json!({
            "active": true,
            "aud": resource,
            "profiles": [{"profile_id": "p1", "permissions": []}]
        });
        let profiles = interpret_introspection(resource, 200, &empty.to_string()).unwrap();
        assert_eq!(entitlement(&profiles).unwrap_err(), UNLICENSED);
        assert_eq!(form_encode("bwa_a b"), "bwa_a%20b");
    }
}
