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

fn get_google_client_id() -> String {
    env!("GOOGLE_CLIENT_ID").to_string()
}

fn get_google_client_secret() -> String {
    env!("GOOGLE_CLIENT_SECRET").to_string()
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

    // 2. Start local HTTP listener on a random port
    let (port, rx) = oauth_server::start_callback_server()?;
    let redirect_uri = format!("http://127.0.0.1:{}", port);

    let client_id = get_google_client_id();

    // 3. Build the authorization URL
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
        AUTH_URL,
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(GMAIL_SCOPE),
        urlencoding::encode(&challenge),
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
        .map_err(|_| anyhow::anyhow!("OAuth callback timed out or was cancelled"))?;

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

fn refresh_access_token(refresh_token: &str) -> Result<String, anyhow::Error> {
    if refresh_token.is_empty() {
        return Err(anyhow::anyhow!(
            "No refresh token found. Please re-connect your Google account in Profile settings."
        ));
    }

    let refresh_token = refresh_token.to_string();
    std::thread::spawn(move || {
        let client_id = get_google_client_id();
        let client_secret = get_google_client_secret();
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(TOKEN_URL)
            .form(&[
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
            ])
            .send()?
            .error_for_status()?;

        let text = resp.text()?;
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
        let client_id = get_google_client_id();
        let client_secret = get_google_client_secret();
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
            return Err(anyhow::anyhow!(
                "Google Token API error ({}): {}",
                status,
                text
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
