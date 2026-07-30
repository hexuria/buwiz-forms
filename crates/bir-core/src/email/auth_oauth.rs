//! Google OAuth2 PKCE authentication for IMAP XOAUTH2.
//!
//! Flow:
//! 1. App opens the browser to Google's consent screen (via `start_oauth_flow`).
//! 2. User approves. Google redirects to a local HTTP listener.
//! 3. App exchanges the auth code for access + refresh tokens.
//! 4. Tokens are stored in the OS Keychain.
//! 5. On subsequent connections, the access token is used for XOAUTH2.
//!    If expired, the refresh token is used to get a new one.

use super::fetcher::ImapAuthenticator;
use sha2::{Digest, Sha256};
use tracing::{info, warn};

fn get_google_client_id() -> Result<String, anyhow::Error> {
    std::env::var("GOOGLE_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("GOOGLE_CLIENT_ID").map(String::from))
        .ok_or_else(|| anyhow::anyhow!("GOOGLE_CLIENT_ID must be set in .env"))
}

fn get_google_client_secret() -> Result<String, anyhow::Error> {
    std::env::var("GOOGLE_CLIENT_SECRET")
        .ok()
        .or_else(|| option_env!("GOOGLE_CLIENT_SECRET").map(String::from))
        .ok_or_else(|| anyhow::anyhow!("GOOGLE_CLIENT_SECRET must be set in .env"))
}

/// Google's OAuth2 endpoints.
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";

/// IMAP scope — required for XOAUTH2 IMAP access + userinfo for email extraction.
const GMAIL_SCOPE: &str = "https://mail.google.com/ https://www.googleapis.com/auth/userinfo.email";

// ── Public API ───────────────────────────────────────────────────────────────

/// IMAP authenticator that uses XOAUTH2 with a Google access token.
pub struct GoogleOAuthAuth {
    email: String,
    access_token: String,
    refresh_token: String,
}

impl GoogleOAuthAuth {
    /// Build an authenticator using the provided tokens.
    pub fn new(
        email: &str,
        access_token: &str,
        refresh_token: &str,
    ) -> Result<Self, anyhow::Error> {
        if access_token.is_empty() && refresh_token.is_empty() {
            return Err(anyhow::anyhow!(
                "No OAuth tokens provided. Please connect your Google account."
            ));
        }

        Ok(Self {
            email: email.to_string(),
            access_token: access_token.to_string(),
            refresh_token: refresh_token.to_string(),
        })
    }
}

impl ImapAuthenticator for GoogleOAuthAuth {
    fn authenticate(
        &self,
        client: imap::Client<native_tls::TlsStream<std::net::TcpStream>>,
    ) -> Result<
        (
            imap::Session<native_tls::TlsStream<std::net::TcpStream>>,
            Option<String>,
        ),
        anyhow::Error,
    > {
        // XOAUTH2 SASL mechanism per https://developers.google.com/gmail/imap/xoauth2-protocol
        // Format: "user={email}\x01auth=Bearer {token}\x01\x01"
        // IMPORTANT: The imap crate's `do_auth_handshake` calls base64::encode() on the
        // response from process(). So we must return RAW bytes, NOT pre-encoded base64.
        let auth_string = format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.email, self.access_token
        );

        match client.authenticate(
            "XOAUTH2",
            &XOAuth2Sasl {
                raw_data: auth_string.into_bytes(),
            },
        ) {
            Ok(session) => Ok((session, None)),
            Err((err, client)) => {
                let error_msg = format!("{:?}", err);
                warn!(
                    "XOAUTH2 authentication failed for {}: {}. Attempting to refresh access token...",
                    self.email, error_msg
                );
                match refresh_access_token(&self.refresh_token) {
                    Ok(new_token) => {
                        let new_auth_string =
                            format!("user={}\x01auth=Bearer {}\x01\x01", self.email, new_token);
                        match client.authenticate(
                            "XOAUTH2",
                            &XOAuth2Sasl {
                                raw_data: new_auth_string.into_bytes(),
                            },
                        ) {
                            Ok(session) => Ok((session, Some(new_token))),
                            Err((e, _)) => Err(anyhow::anyhow!(
                                "XOAUTH2 authentication failed even after token refresh. Please reconnect your Google Account. Error: {:?}",
                                e
                            )),
                        }
                    }
                    Err(refresh_err) => Err(anyhow::anyhow!(
                        "XOAUTH2 authentication failed and token refresh also failed: {}",
                        refresh_err
                    )),
                }
            }
        }
    }

    fn host(&self) -> &str {
        "imap.gmail.com"
    }
}

/// XOAUTH2 SASL authenticator.
///
/// The `imap` crate's `do_auth_handshake` calls `base64::encode()` on whatever
/// `process()` returns. So we return the RAW SASL bytes — NOT pre-encoded.
struct XOAuth2Sasl {
    raw_data: Vec<u8>,
}

impl imap::Authenticator for XOAuth2Sasl {
    type Response = Vec<u8>;
    fn process(&self, _challenge: &[u8]) -> Self::Response {
        self.raw_data.clone()
    }
}

// ── OAuth2 Flow ──────────────────────────────────────────────────────────────

/// Kick off the browser-based OAuth2 PKCE flow.
///
/// Returns `(email, access_token, refresh_token)` on success.
/// This is called from the UI thread when the user clicks "Connect Google Account".
pub fn start_oauth_flow() -> Result<(String, String, String), anyhow::Error> {
    use super::oauth_server;

    // 1. Generate PKCE verifier + challenge
    let verifier = generate_pkce_verifier();
    let challenge = generate_pkce_challenge(&verifier);
    let state = generate_pkce_verifier();

    // 2. Start local HTTP listener on a random port
    let (port, rx) = oauth_server::start_callback_server(state.clone())?;
    let redirect_uri = format!("http://127.0.0.1:{}", port);

    let client_id = get_google_client_id()?;

    // 3. Build the authorization URL
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&state={}&access_type=offline&prompt=consent",
        AUTH_URL,
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(GMAIL_SCOPE),
        urlencoding::encode(&challenge),
        urlencoding::encode(&state),
    );

    // 4. Open the browser
    info!("Opening browser for Google OAuth2 consent...");
    if let Err(e) = open::that(&auth_url) {
        warn!(
            "Failed to open browser: {}. Please open this URL manually:\n{}",
            e, auth_url
        );
    }

    // 5. Wait for the callback (blocks until user approves or timeout)
    let code = rx
        .recv()
        .map_err(|_| anyhow::anyhow!("OAuth callback timed out or was cancelled"))?
        .map_err(anyhow::Error::msg)?;

    // 6. Exchange the code for tokens
    let (access_token, refresh_token) = exchange_code_for_tokens(&code, &verifier, &redirect_uri)?;

    let access_token_clone = access_token.clone();
    let email = std::thread::spawn(move || -> Result<String, anyhow::Error> {
        let client = reqwest::blocking::Client::new();
        let userinfo_resp = client
            .get(USERINFO_URL)
            .header("Authorization", format!("Bearer {}", access_token_clone))
            .send()?;

        let userinfo_status = userinfo_resp.status();
        let userinfo_text = userinfo_resp.text()?;
        if !userinfo_status.is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch user email ({}): {}",
                userinfo_status,
                userinfo_text
            ));
        }

        let userinfo: serde_json::Value = serde_json::from_str(&userinfo_text)?;
        let email = userinfo
            .get("email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("No email found in userinfo response"))?
            .to_string();
        Ok(email)
    })
    .join()
    .unwrap_or_else(|_| Err(anyhow::anyhow!("Thread panicked")))?;

    Ok((email, access_token, refresh_token))
}

/// Get the email address associated with the stored OAuth token (from Google's userinfo endpoint).
pub fn get_oauth_email(access_token: &str) -> Result<String, anyhow::Error> {
    let access_token = access_token.to_string();
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let resp = client
            .get("https://www.googleapis.com/oauth2/v3/userinfo")
            .bearer_auth(access_token)
            .send()?
            .error_for_status()?;

        let text = resp.text()?;
        let info: serde_json::Value = serde_json::from_str(&text)?;
        info.get("email")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Could not retrieve email from Google userinfo"))
    })
    .join()
    .unwrap_or_else(|_| Err(anyhow::anyhow!("Thread panicked")))
}

// ── Token Management ─────────────────────────────────────────────────────────

/// Turns a failed Google token-endpoint response into something a user can act on.
///
/// Google returns the real reason as `error` / `error_description` in the JSON
/// body. The status line alone cannot distinguish "your token expired, reconnect"
/// from "your OAuth client is misconfigured, and reconnecting will not help" -
/// both are plain `400 Bad Request`. Reporting only the status sent us chasing a
/// non-existent bug in the refresh logic, which fires correctly; it is the token
/// that was dead.
fn describe_token_endpoint_failure(status: u16, body: &str) -> String {
    let parsed: Option<serde_json::Value> = serde_json::from_str(body).ok();
    let code = parsed
        .as_ref()
        .and_then(|v| v.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let description = parsed
        .as_ref()
        .and_then(|v| v.get("error_description"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    // Guidance per documented Google error code. `invalid_grant` is by far the
    // most common and has a cause worth naming: an OAuth app left in "Testing"
    // publishing status has its refresh tokens expired by Google every 7 days,
    // so reconnecting works but only until the next expiry.
    let guidance = match code {
        "invalid_grant" => {
            "The refresh token is expired or revoked. Reconnect your Google account in \
             Profile > Email Settings. If this recurs about weekly, the OAuth app is \
             likely still in \"Testing\" publishing status in Google Cloud Console, \
             which expires refresh tokens every 7 days - publishing it stops that."
        }
        "invalid_client" | "unauthorized_client" => {
            "The configured Google client ID or secret does not match the one that \
             issued this token. Reconnecting will not help until the credentials are \
             corrected."
        }
        "invalid_scope" => {
            "The stored token lacks the scopes this app now requires. Reconnect your \
             Google account to re-consent."
        }
        _ => "Reconnect your Google account in Profile > Email Settings if this persists.",
    };

    let reported = match (code.is_empty(), description.is_empty()) {
        (false, false) => format!("{code}: {description}"),
        (false, true) => code.to_string(),
        // No parseable error object; include a bounded slice of the raw body so
        // the cause is not lost entirely.
        (true, _) => {
            let raw = body.trim();
            if raw.is_empty() {
                format!("HTTP {status} with an empty body")
            } else {
                // Truncate on a char boundary. Slicing by byte index panics when
                // it lands mid-character, and a proxy or captive portal can
                // return a multi-byte HTML error page here.
                let end = raw
                    .char_indices()
                    .map(|(i, c)| i + c.len_utf8())
                    .take_while(|&i| i <= 200)
                    .last()
                    .unwrap_or(0);
                format!("HTTP {status}: {}", &raw[..end])
            }
        }
    };

    format!("Google rejected the token refresh - {reported}. {guidance}")
}

fn refresh_access_token(refresh_token: &str) -> Result<String, anyhow::Error> {
    if refresh_token.is_empty() {
        return Err(anyhow::anyhow!(
            "No refresh token found. Please re-connect your Google account in Profile settings."
        ));
    }

    let refresh_token = refresh_token.to_string();
    std::thread::spawn(move || {
        let client_id = get_google_client_id()?;
        let client_secret = get_google_client_secret()?;
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(TOKEN_URL)
            .form(&[
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
            ])
            .send()?;

        // Deliberately NOT `error_for_status()`. That discards the response
        // body, which is the only place Google says what actually went wrong,
        // leaving the caller with a bare "400 Bad Request" and no way to tell
        // an expired token from a misconfigured client.
        let status = resp.status();
        let text = resp.text()?;
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "{}",
                describe_token_endpoint_failure(status.as_u16(), &text)
            ));
        }

        let body: serde_json::Value = serde_json::from_str(&text)?;
        let access_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Google token refresh did not return an access_token"))?
            .to_string();

        info!("Refreshed Google OAuth access token successfully");
        Ok(access_token)
    })
    .join()
    .unwrap_or_else(|_| Err(anyhow::anyhow!("Thread panicked")))
}

fn exchange_code_for_tokens(
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<(String, String), anyhow::Error> {
    let code = code.to_string();
    let verifier = verifier.to_string();
    let redirect_uri = redirect_uri.to_string();

    std::thread::spawn(move || -> Result<(String, String), anyhow::Error> {
        let client_id = get_google_client_id()?;
        let client_secret = get_google_client_secret()?;
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(TOKEN_URL)
            .form(&[
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("code", code.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
                ("grant_type", "authorization_code"),
                ("code_verifier", verifier.as_str()),
            ])
            .send()?;

        let status = resp.status();
        let text = resp.text()?;
        if !status.is_success() {
            // Same endpoint as the refresh path, so the same interpretation
            // applies - this previously dumped the raw JSON with no guidance.
            return Err(anyhow::anyhow!(
                "{}",
                describe_token_endpoint_failure(status.as_u16(), &text)
            ));
        }

        let body: serde_json::Value = serde_json::from_str(&text)?;

        let access_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing access_token in token response"))?
            .to_string();

        let refresh_token = body
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing refresh_token in token response"))?
            .to_string();

        Ok((access_token, refresh_token))
    })
    .join()
    .unwrap_or_else(|_| Err(anyhow::anyhow!("Thread panicked")))
}

// ── PKCE Helpers ─────────────────────────────────────────────────────────────

fn generate_pkce_verifier() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
    data_encoding::BASE64URL_NOPAD.encode(&bytes)
}

fn generate_pkce_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    data_encoding::BASE64URL_NOPAD.encode(&hash)
}

#[cfg(test)]
mod token_endpoint_failure_tests {
    use super::describe_token_endpoint_failure;

    // Payloads below are the shapes Google actually returns from
    // https://oauth2.googleapis.com/token. Before this change every one of them
    // surfaced as a bare "400 Bad Request", which is why an expired token and a
    // misconfigured client were indistinguishable in the log.

    #[test]
    fn expired_or_revoked_token_names_the_cause_and_the_weekly_trap() {
        let msg = describe_token_endpoint_failure(
            400,
            r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#,
        );
        assert!(msg.contains("invalid_grant"), "{msg}");
        assert!(msg.contains("Token has been expired or revoked"), "{msg}");
        assert!(
            msg.contains("Email Settings"),
            "must say where to reconnect: {msg}"
        );
        // The recurring-weekly cause is the whole point of this message.
        assert!(
            msg.contains("Testing"),
            "must name the 7-day expiry cause: {msg}"
        );
    }

    #[test]
    fn client_misconfiguration_says_reconnecting_will_not_help() {
        for code in ["invalid_client", "unauthorized_client"] {
            let msg = describe_token_endpoint_failure(
                401,
                &format!(
                    r#"{{"error":"{code}","error_description":"The OAuth client was not found."}}"#
                ),
            );
            assert!(msg.contains(code), "{msg}");
            assert!(
                msg.contains("will not help"),
                "reconnecting cannot fix bad credentials, and the message must say so: {msg}"
            );
        }
    }

    #[test]
    fn scope_change_asks_for_reconsent() {
        let msg = describe_token_endpoint_failure(400, r#"{"error":"invalid_scope"}"#);
        assert!(msg.contains("invalid_scope"), "{msg}");
        assert!(msg.contains("re-consent"), "{msg}");
    }

    #[test]
    fn a_non_json_body_still_reports_something_useful() {
        let msg = describe_token_endpoint_failure(502, "<html>Bad Gateway</html>");
        assert!(msg.contains("502"), "{msg}");
        assert!(
            msg.contains("Bad Gateway"),
            "the raw body must not be lost: {msg}"
        );
    }

    #[test]
    fn an_empty_body_does_not_produce_a_dangling_message() {
        let msg = describe_token_endpoint_failure(400, "");
        assert!(msg.contains("400"), "{msg}");
        assert!(msg.contains("empty body"), "{msg}");
    }

    #[test]
    fn an_enormous_body_is_bounded() {
        let msg = describe_token_endpoint_failure(400, &"x".repeat(10_000));
        assert!(
            msg.len() < 600,
            "a huge body must not flood the log: {}",
            msg.len()
        );
    }

    #[test]
    fn a_multibyte_body_is_truncated_on_a_char_boundary() {
        // Slicing by byte index panics mid-character; the guard must not.
        let msg = describe_token_endpoint_failure(400, &"日本語テキスト".repeat(100));
        assert!(msg.contains("400"), "{msg}");
    }
}
