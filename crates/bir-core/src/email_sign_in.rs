//! Signing in inside the app with an email address and a six-digit code (`POST /auth/email/start`, `/resend`,
//! `/verify`), behind the same [`Http`] seam as the device flow, and the pending request that survives the app being
//! closed.
//!
//! The app makes a 32-byte secret per request and sends only its hash at the start; the code is checked with the
//! request id and the secret, so a code from another request's mail (another device's) is `invalid_code` here. The
//! verify answer is exactly the device flow's token response.
//!
//! The pending request `{server_url, request_id, secret, email, expires_at, resend_at, intent, code_length}` is a
//! credential. It is kept where the saved sign-in is kept: on macOS, Windows and Linux the OS credential store, with a
//! mode-0600 file beside the database when that store is off or refuses (`BUWIZ_NO_KEYCHAIN`); on iPhone, iPad and
//! Android the file beside the database, which goes with the app. It is never logged.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::cloud_account::{
    CloudError, DESKTOP_CLIENT_ID, Http, HttpRequest, KEYRING_SERVICE, TokenSet,
};

/// The OS credential store's account of the pending request (beside [`crate::cloud_account::KEYRING_ACCOUNT`]).
pub const KEYRING_PENDING_ACCOUNT: &str = "pending-sign-in";
/// The fallback file beside the database.
pub const PENDING_FILE_NAME: &str = "buwiz-pending-sign-in.json";
/// Overrides the fallback file (tests).
pub const ENV_PENDING_FILE: &str = "BUWIZ_PENDING_SIGN_IN_FILE";

/// What the person asked for. The server makes no difference; the app sends `sign_in`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    SignIn,
    Create,
}

impl Intent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SignIn => "sign_in",
            Self::Create => "create",
        }
    }
}

/// Which request a guard token is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardFor {
    Start,
    Resend,
}

/// A bot check's answer for one request: a Turnstile token, an app attestation. Neither is made yet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Guard {
    pub turnstile: Option<String>,
    pub attestation: Option<String>,
}

/// The one seam for `turnstile` and `attestation`: the guard for a request, if any. Nothing fills it yet (the
/// Turnstile view and app attestation are the next change), so no request carries either.
pub fn guard_for(_request: GuardFor) -> Option<Guard> {
    None
}

/// A fresh secret: 32 random bytes, base64url without padding (what `/resend` and `/verify` send).
pub fn new_secret() -> String {
    data_encoding::BASE64URL_NOPAD.encode(&rand::random::<[u8; 32]>())
}

/// `base64url(SHA-256(secret))` without padding, over the secret string's bytes, as PKCE S256 (what `/start` sends).
pub fn secret_hash(secret: &str) -> String {
    data_encoding::BASE64URL_NOPAD.encode(&Sha256::digest(secret.as_bytes()))
}

/// The answer of `/start` and `/resend`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmailStarted {
    pub request_id: String,
    pub expires_in: i64,
    pub resend_after: i64,
    pub code_length: u32,
}

/// Why a step of the email sign-in did not go on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmailError {
    /// This server does not offer the email sign-in (404, HTML, a redirect: see `not_offered`). The email page says
    /// sign-in is not available; the app never switches to the browser by itself.
    NotOffered,
    /// The server said no, with its code (`invalid_code`, `code_expired`, ...) and the sentence to show.
    /// A 429 that is not the contract's JSON: the network's outer limit (more than 10 requests in 10 seconds). Not
    /// tried again by itself.
    Throttled,
    Refused {
        code: String,
        message: String,
        retry_after: Option<i64>,
        attempts_left: Option<i64>,
    },
    Network(String),
    Unexpected(String),
}

impl EmailError {
    /// The code of a refusal, `""` otherwise.
    pub fn code(&self) -> &str {
        match self {
            Self::Refused { code, .. } => code,
            _ => "",
        }
    }

    /// The sentence the app shows: the server's message when it gave one, else this app's own for the code. Never a
    /// raw code or JSON. Where the answer carries the number the person acts on (the tries left of `invalid_code`, the
    /// wait of `code_already_sent` and `restarting`), this app's own sentence says it, whatever the message: the
    /// contract's messages need not name it ("That code is not right." alone left a phone without its count).
    pub fn sentence(&self) -> String {
        match self {
            Self::NotOffered => {
                "Sign-in is not available right now. Try again in a few minutes.".into()
            }
            Self::Throttled => "Too many tries. Wait a moment and try again.".into(),
            Self::Refused {
                code,
                retry_after,
                attempts_left,
                ..
            } if own_sentence(code, *retry_after, *attempts_left) => {
                local_sentence(code, *retry_after, *attempts_left)
            }
            Self::Refused { message, .. } if !message.trim().is_empty() => {
                message.trim().to_owned()
            }
            Self::Refused {
                code,
                retry_after,
                attempts_left,
                ..
            } => local_sentence(code, *retry_after, *attempts_left),
            Self::Network(_) => "No connection. Check your internet and try again.".into(),
            Self::Unexpected(_) => {
                "The Buwiz server gave an answer this app does not understand. Try again.".into()
            }
        }
    }
}

impl EmailError {
    /// The sentence on the code page: as [`EmailError::sentence`], but a `too_many_requests` there is the server's limit
    /// on code checks (300 an hour from one network, or the shared budget past 100k networks), which spends none of the
    /// request's tries: "Too many sign-in codes are being checked right now. Try again in N minutes." (in a minute
    /// under 90 seconds).
    pub fn verify_sentence(&self) -> String {
        match self {
            Self::Refused {
                code, retry_after, ..
            } if code == "too_many_requests" => {
                let wait = match retry_after {
                    Some(s) if *s >= 90 => format!("in {} minutes", (s + 59) / 60),
                    _ => "in a minute".into(),
                };
                format!("Too many sign-in codes are being checked right now. Try again {wait}.")
            }
            _ => self.sentence(),
        }
    }
}

impl std::fmt::Display for EmailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.sentence())
    }
}

impl std::error::Error for EmailError {}

/// Whether this app's sentence for `code` is said whatever the message (see [`EmailError::sentence`]): it carries a
/// number the answer gave, or the server's message may be untrue for this case (`too_many_requests` is the address's
/// limit or the network's, and the answer does not say which; `busy` is said as one sentence everywhere).
fn own_sentence(code: &str, retry_after: Option<i64>, attempts_left: Option<i64>) -> bool {
    match code {
        "invalid_code" => attempts_left.is_some_and(|n| n > 0),
        "code_already_sent" | "restarting" => retry_after.is_some_and(|s| s > 0),
        "too_many_requests" | "busy" => true,
        _ => false,
    }
}

/// "in 3 minutes", "in a minute", "in 40 seconds".
fn wait_words(seconds: Option<i64>) -> String {
    match seconds {
        Some(s) if s >= 90 => format!("in {} minutes", (s + 59) / 60),
        Some(s) if s > 45 => "in a minute".into(),
        Some(s) if s > 0 => format!("in {s} seconds"),
        _ => "in a minute".into(),
    }
}

/// "5 minutes", "1 minute": the server's wait in whole minutes, rounded up (a minute when it named none).
fn minutes(seconds: Option<i64>) -> String {
    let n = seconds.map_or(1, |s| ((s.max(1) + 59) / 60).max(1));
    if n == 1 {
        "1 minute".into()
    } else {
        format!("{n} minutes")
    }
}

/// This app's sentence for each code of the contract, for an answer without a `message`.
pub fn local_sentence(code: &str, retry_after: Option<i64>, attempts_left: Option<i64>) -> String {
    match code {
        "invalid_email" => "That email address does not look right. Check it and try again.".into(),
        "too_many_requests" => format!(
            "Too many sign-in codes were asked for. Try again in {}.",
            minutes(retry_after)
        ),
        "code_already_sent" => format!(
            "A sign-in code was already sent to this address. You can ask for a new one {}.",
            wait_words(retry_after)
        ),
        "turnstile_required" | "turnstile_failed" => {
            "Sign-in could not be checked on this device. Try again.".into()
        }
        "mail_failed" => "The email could not be sent. Check the address and try again.".into(),
        "bad_install_id" => "Sign-in could not be checked on this device. Try again.".into(),
        "unknown_client" => {
            "This app is not known to the Buwiz server. Update the app and try again.".into()
        }
        "busy" => "Sign-in is busy right now. Try again in a moment.".into(),
        "restarting" => format!(
            "The Buwiz server is restarting. Try again {}.",
            wait_words(retry_after)
        ),
        "invalid_code" => match attempts_left {
            Some(1) => "That code is not right. 1 try left.".into(),
            Some(n) if n > 1 => format!("That code is not right. {n} tries left."),
            _ => "That code is not right. Check the email and try again.".into(),
        },
        "code_expired" => "This code has expired. Send a new code to try again.".into(),
        "too_many_attempts" => "Too many wrong codes. Send a new code to try again.".into(),
        "link_used" => "That sign-in link was already used. Start again.".into(),
        "invalid_request" => "This sign-in request has ended. Start again.".into(),
        "install_mismatch" => "This code was requested on another device. Start again here.".into(),
        "account_banned" => "This Buwiz account is closed. Write to support@buwiz.com.".into(),
        _ => "The Buwiz server refused the sign-in. Try again.".into(),
    }
}

/// Every `error` the contract answers on these paths.
const CONTRACT_CODES: [&str; 18] = [
    "install_mismatch",
    "attestation_refused",
    "invalid_request",
    "unknown_client",
    "invalid_email",
    "bad_install_id",
    "code_expired",
    "invalid_code",
    "too_many_attempts",
    "link_used",
    "account_banned",
    "turnstile_failed",
    "turnstile_required",
    "code_already_sent",
    "too_many_requests",
    "mail_failed",
    "restarting",
    "busy",
];

/// Whether an answer on any of the three paths is not this contract's: a 404, or anything that is neither a 200 JSON
/// object nor one of the contract's errors (`{"error": <a contract code>}`): an older server answering the web form, a
/// challenge page, a proxy's error page, a redirect. The page says sign-in is not available right now and the person
/// stays where they are. `mail_failed` (502) and `restarting` (503) are the contract's: not this.
fn not_offered(status: u16, value: &Value) -> bool {
    if status == 404 {
        return true;
    }
    let code = value.get("error").and_then(Value::as_str);
    if status == 200 {
        return !value.is_object() || code.is_some();
    }
    !code.is_some_and(|code| CONTRACT_CODES.contains(&code))
}

fn post(
    http: &mut dyn Http,
    server_url: &str,
    path: &str,
    body: Value,
) -> Result<(u16, Value), EmailError> {
    let response = http
        .exchange(&HttpRequest {
            method: "POST",
            url: format!("{server_url}{path}"),
            content_type: "application/json",
            bearer: None,
            body: body.to_string(),
        })
        .map_err(|error| match error {
            CloudError::Network(message) => EmailError::Network(message),
            other => EmailError::Unexpected(other.to_string()),
        })?;
    let value: Value = serde_json::from_str(&response.body).unwrap_or(Value::Null);
    if not_offered(response.status, &value) {
        // An outer rate limit answers its own page, not the server's JSON 429.
        if response.status == 429 {
            return Err(EmailError::Throttled);
        }
        return Err(EmailError::NotOffered);
    }
    if !(200..300).contains(&response.status) || value.get("error").is_some_and(|e| !e.is_null()) {
        let number = |key: &str| value.get(key).and_then(Value::as_i64);
        return Err(EmailError::Refused {
            code: value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("http_{}", response.status)),
            message: value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            retry_after: number("retry_after"),
            attempts_left: number("attempts_left"),
        });
    }
    if value.is_null() {
        return Err(EmailError::Unexpected("not JSON".into()));
    }
    Ok((response.status, value))
}

fn with_guard(mut body: Value, guard: Option<Guard>) -> Value {
    if let Some(guard) = guard {
        if let Some(token) = guard.turnstile {
            body["turnstile"] = json!(token);
        }
        if let Some(token) = guard.attestation {
            body["attestation"] = json!(token);
        }
    }
    body
}

fn started(value: &Value) -> Result<EmailStarted, EmailError> {
    let request_id = value
        .get("request_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| EmailError::Unexpected("no request_id".into()))?
        .to_owned();
    let seconds = |key: &str| value.get(key).and_then(Value::as_i64);
    let expires_in = seconds("expires_in")
        .filter(|s| *s > 0)
        .ok_or_else(|| EmailError::Unexpected("no expires_in".into()))?;
    Ok(EmailStarted {
        request_id,
        expires_in,
        // The contract makes Resend wait for the code's expiry; an answer without it waits as long.
        resend_after: seconds("resend_after")
            .filter(|s| *s >= 0)
            .unwrap_or(expires_in),
        code_length: value
            .get("code_length")
            .and_then(Value::as_u64)
            .filter(|n| (4..=10).contains(n))
            .unwrap_or(6) as u32,
    })
}

/// `POST /auth/email/start`. The answer is the same whether or not the address has an account.
#[allow(clippy::too_many_arguments)]
pub fn start(
    http: &mut dyn Http,
    server_url: &str,
    email: &str,
    intent: Intent,
    secret: &str,
    platform: &str,
    install_id: &str,
    guard: Option<Guard>,
) -> Result<EmailStarted, EmailError> {
    let body = with_guard(
        json!({
            "client_id": DESKTOP_CLIENT_ID,
            "email": email,
            "intent": intent.as_str(),
            "secret_hash": secret_hash(secret),
            "platform": platform,
            "install_id": install_id,
        }),
        guard,
    );
    let (_, value) = post(http, server_url, "/auth/email/start", body)?;
    // A 200 JSON without the request is not this contract either.
    started(&value).map_err(|_| EmailError::NotOffered)
}

/// `POST /auth/email/resend`: a new code; the old code and link stop working. Only once the code has expired.
pub fn resend(
    http: &mut dyn Http,
    server_url: &str,
    request_id: &str,
    secret: &str,
    guard: Option<Guard>,
) -> Result<EmailStarted, EmailError> {
    let body = with_guard(json!({ "request_id": request_id, "secret": secret }), guard);
    let (_, value) = post(http, server_url, "/auth/email/resend", body)?;
    started(&value)
}

/// `POST /auth/email/verify` with the code: the device flow's token response. The request is then spent.
pub fn verify(
    http: &mut dyn Http,
    server_url: &str,
    request_id: &str,
    secret: &str,
    code: &str,
) -> Result<TokenSet, EmailError> {
    let (_, value) = post(
        http,
        server_url,
        "/auth/email/verify",
        json!({ "request_id": request_id, "secret": secret, "code": code }),
    )?;
    let access_token = value
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| EmailError::Unexpected("no access_token".into()))?
        .to_owned();
    Ok(TokenSet {
        access_token,
        refresh_token: value
            .get("refresh_token")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        expires_in: value
            .get("expires_in")
            .and_then(Value::as_i64)
            .unwrap_or(900)
            .max(1),
    })
}

// ------------------------------------------------------------------ the pending request

/// The email sign-in waiting for its code. A credential: [`std::fmt::Debug`] hides the secret.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PendingEmail {
    pub server_url: String,
    pub request_id: String,
    pub secret: String,
    pub email: String,
    pub expires_at_unix: i64,
    pub resend_at_unix: i64,
    pub intent: Intent,
    #[serde(default = "six")]
    pub code_length: u32,
}

fn six() -> u32 {
    6
}

impl std::fmt::Debug for PendingEmail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingEmail")
            .field("server_url", &self.server_url)
            .field("expires_at_unix", &self.expires_at_unix)
            .field("resend_at_unix", &self.resend_at_unix)
            .field("request_id", &"<redacted>")
            .field("secret", &"<redacted>")
            .field("email", &"<redacted>")
            .finish()
    }
}

impl PendingEmail {
    /// The request as the server answered it, at `now_unix`.
    pub fn from_answer(
        server_url: &str,
        email: &str,
        intent: Intent,
        secret: String,
        answer: &EmailStarted,
        now_unix: i64,
    ) -> Self {
        Self {
            server_url: server_url.to_owned(),
            request_id: answer.request_id.clone(),
            secret,
            email: email.to_owned(),
            expires_at_unix: now_unix.saturating_add(answer.expires_in),
            resend_at_unix: now_unix.saturating_add(answer.resend_after),
            intent,
            code_length: answer.code_length,
        }
    }

    /// A Resend's answer: a new life for the same request.
    pub fn renewed(&self, answer: &EmailStarted, now_unix: i64) -> Self {
        let mut next = self.clone();
        next.request_id = answer.request_id.clone();
        next.expires_at_unix = now_unix.saturating_add(answer.expires_in);
        next.resend_at_unix = now_unix.saturating_add(answer.resend_after);
        next.code_length = answer.code_length;
        next
    }

    pub fn expired(&self, now_unix: i64) -> bool {
        now_unix >= self.expires_at_unix
    }

    fn decode(raw: &str) -> Option<Self> {
        serde_json::from_str::<Self>(raw).ok().filter(|p| {
            !p.request_id.is_empty() && !p.secret.is_empty() && !p.server_url.is_empty()
        })
    }
}

/// The pending request, if one is kept (the credential store first, then the file).
pub fn load_pending() -> Option<PendingEmail> {
    if let Some(found) = keyring_load().as_deref().and_then(PendingEmail::decode) {
        return Some(found);
    }
    std::fs::read_to_string(pending_file_path())
        .ok()
        .as_deref()
        .and_then(PendingEmail::decode)
}

/// Keeps the pending request: in the credential store, or in the file when the store is off or refuses.
pub fn save_pending(pending: &PendingEmail) -> Result<(), String> {
    let raw = serde_json::to_string(pending).map_err(|error| error.to_string())?;
    if keyring_save(&raw) {
        let _ = std::fs::remove_file(pending_file_path());
        return Ok(());
    }
    write_private(&pending_file_path(), &raw)
        .map_err(|error| format!("the sign-in request could not be kept ({error})"))
}

/// Forgets the pending request everywhere it may be.
pub fn clear_pending() {
    keyring_delete();
    let _ = std::fs::remove_file(pending_file_path());
}

/// The pending email sign-in's file: in the device folder (`platform::device_dir`), whichever room is open.
pub(crate) fn pending_file_path() -> PathBuf {
    if let Some(path) = std::env::var_os(ENV_PENDING_FILE) {
        return PathBuf::from(path);
    }
    crate::platform::device_dir().join(PENDING_FILE_NAME)
}

fn write_private(path: &Path, raw: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(raw.as_bytes())
}

// Under `BUWIZ_NO_KEYCHAIN=1` none of these reaches the OS: a read finds nothing, a write fails (the file is used).

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn keyring_load() -> Option<String> {
    if crate::platform::keychain_disabled() {
        return None;
    }
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_PENDING_ACCOUNT)
        .ok()?
        .get_password()
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn keyring_save(raw: &str) -> bool {
    if crate::platform::keychain_disabled() {
        return false;
    }
    // Read back: a keyring with no real backend reports success and forgets.
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_PENDING_ACCOUNT)
        .and_then(|entry| entry.set_password(raw))
        .is_ok()
        && keyring_load().as_deref() == Some(raw)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn keyring_delete() {
    if crate::platform::keychain_disabled() {
        return;
    }
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_PENDING_ACCOUNT) {
        let _ = entry.delete_credential();
    }
}

// iPhone, iPad and Android: the file beside the database only, as the saved sign-in there. Deleting the app deletes it.

#[cfg(any(target_os = "android", target_os = "ios"))]
fn keyring_load() -> Option<String> {
    let _ = KEYRING_SERVICE;
    None
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn keyring_save(_raw: &str) -> bool {
    false
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn keyring_delete() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_account::HttpResponse;

    /// A fake server: the answers in order, and the requests it was sent.
    struct Fake {
        answers: Vec<(u16, String)>,
        sent: Vec<(String, Value)>,
    }

    impl Fake {
        fn answering(answers: &[(u16, &str)]) -> Self {
            Self {
                answers: answers
                    .iter()
                    .rev()
                    .map(|(s, b)| (*s, (*b).to_owned()))
                    .collect(),
                sent: Vec::new(),
            }
        }
    }

    impl Http for Fake {
        fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
            self.sent.push((
                request.url.clone(),
                serde_json::from_str(&request.body).unwrap_or(Value::Null),
            ));
            let (status, body) = self
                .answers
                .pop()
                .ok_or_else(|| CloudError::Network("down".into()))?;
            Ok(HttpResponse { status, body })
        }
    }

    const STARTED: &str =
        r#"{"request_id":"r1","expires_in":300,"resend_after":300,"code_length":6}"#;

    fn start_with(fake: &mut Fake) -> Result<EmailStarted, EmailError> {
        start(
            fake,
            "http://127.0.0.1:9",
            "filer@example.test",
            Intent::SignIn,
            &new_secret(),
            "macos",
            "install-id-of-this-test-0001",
            guard_for(GuardFor::Start),
        )
    }

    #[test]
    fn the_secret_is_32_random_bytes_and_only_its_hash_is_sent_at_the_start() {
        let secret = new_secret();
        assert_eq!(
            data_encoding::BASE64URL_NOPAD
                .decode(secret.as_bytes())
                .unwrap()
                .len(),
            32
        );
        assert_ne!(secret, new_secret());
        let mut fake = Fake::answering(&[(200, STARTED)]);
        start(
            &mut fake,
            "http://127.0.0.1:9",
            "filer@example.test",
            Intent::SignIn,
            &secret,
            "ios",
            "install-id-of-this-test-0001",
            None,
        )
        .unwrap();
        let (url, body) = &fake.sent[0];
        assert_eq!(url, "http://127.0.0.1:9/auth/email/start");
        assert_eq!(body["client_id"], "buwiz-desktop");
        assert_eq!(body["intent"], "sign_in");
        assert_eq!(body["platform"], "ios");
        assert_eq!(body["install_id"], "install-id-of-this-test-0001");
        assert_eq!(body["secret_hash"], json!(secret_hash(&secret)));
        assert!(
            !body.to_string().contains(&secret),
            "the secret itself never goes at the start"
        );
        assert!(body.get("turnstile").is_none() && body.get("attestation").is_none());
        assert_eq!(secret_hash(&secret).len(), 43);
        // As PKCE S256: the hash of the string's bytes (the value `openssl dgst -sha256 -binary`, base64url, gives).
        assert_eq!(
            secret_hash("dBjftJeZ4CVP-mJ92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "ngF5GsXcbwljx6u133FFr3Xht9xooA_DuaX_3QwODtc"
        );
    }

    #[test]
    fn the_guard_seam_is_empty_and_its_tokens_would_be_sent() {
        assert_eq!(guard_for(GuardFor::Start), None);
        assert_eq!(guard_for(GuardFor::Resend), None);
        let body = with_guard(
            json!({}),
            Some(Guard {
                turnstile: Some("t".into()),
                attestation: Some("a".into()),
            }),
        );
        assert_eq!(body, json!({"turnstile":"t","attestation":"a"}));
    }

    #[test]
    fn the_answer_is_read_from_the_server_and_nothing_is_hard_coded() {
        let mut fake = Fake::answering(&[(
            200,
            r#"{"request_id":"r9","expires_in":120,"resend_after":90,"code_length":6}"#,
        )]);
        assert_eq!(
            start_with(&mut fake).unwrap(),
            EmailStarted {
                request_id: "r9".into(),
                expires_in: 120,
                resend_after: 90,
                code_length: 6
            }
        );
        let mut fake = Fake::answering(&[(200, r#"{"request_id":"r9","expires_in":120}"#)]);
        let answer = start_with(&mut fake).unwrap();
        assert_eq!((answer.resend_after, answer.code_length), (120, 6));
        for bad in [r#"{"expires_in":300}"#, r#"{"request_id":"r"}"#, "not json"] {
            let mut fake = Fake::answering(&[(200, bad)]);
            assert!(
                matches!(start_with(&mut fake), Err(EmailError::NotOffered)),
                "{bad}"
            );
        }
    }

    #[test]
    fn an_answer_outside_the_contract_means_the_server_does_not_offer_it() {
        for (status, body) in [
            (404, r#"{"error":"not_found"}"#),
            (404, ""),
            (200, "<!doctype html><form action=/auth/email/start>"),
            (200, r#"{"ok":true}"#),
            (200, r#"["not","an","object"]"#),
            (302, ""),
            (303, r#"{"error":"see_other"}"#),
            (400, "<html>Bad Request</html>"),
            (405, r#"{"error":"method_not_allowed"}"#),
            (500, "Internal Server Error"),
            (500, r#"{"error":"something_new"}"#),
        ] {
            let mut fake = Fake::answering(&[(status, body)]);
            assert_eq!(
                start_with(&mut fake),
                Err(EmailError::NotOffered),
                "{status} {body}"
            );
        }
        assert_eq!(
            EmailError::NotOffered.sentence(),
            "Sign-in is not available right now. Try again in a few minutes."
        );
        // The contract's own server errors are said, not taken for "not offered".
        for (status, body) in [
            (502, r#"{"error":"mail_failed","message":"x"}"#),
            (503, r#"{"error":"restarting","retry_after":30}"#),
        ] {
            let mut fake = Fake::answering(&[(status, body)]);
            assert!(
                matches!(start_with(&mut fake), Err(EmailError::Refused { .. })),
                "{body}"
            );
        }
        // A network failure is not "not offered" either.
        assert!(matches!(
            start_with(&mut Fake::answering(&[])),
            Err(EmailError::Network(_))
        ));
    }

    #[test]
    fn every_start_and_resend_error_has_a_plain_sentence() {
        let cases: [(u16, &str, &str); 9] = [
            (
                400,
                "invalid_email",
                "That email address does not look right. Check it and try again.",
            ),
            (
                429,
                r#"too_many_requests","retry_after":120"#,
                "Too many sign-in codes were asked for. Try again in 2 minutes.",
            ),
            (
                429,
                r#"code_already_sent","retry_after":240"#,
                "A sign-in code was already sent to this address. You can ask for a new one in 4 minutes.",
            ),
            (
                403,
                "turnstile_required",
                "Sign-in could not be checked on this device. Try again.",
            ),
            (
                403,
                "turnstile_failed",
                "Sign-in could not be checked on this device. Try again.",
            ),
            (
                400,
                "bad_install_id",
                "Sign-in could not be checked on this device. Try again.",
            ),
            (
                502,
                "mail_failed",
                "The email could not be sent. Check the address and try again.",
            ),
            (
                400,
                "unknown_client",
                "This app is not known to the Buwiz server. Update the app and try again.",
            ),
            (
                503,
                r#"restarting","retry_after":30"#,
                "The Buwiz server is restarting. Try again in 30 seconds.",
            ),
        ];
        for (status, error, sentence) in cases {
            let body = format!(r#"{{"error":"{error}}}"#);
            let body = if error.contains('"') {
                body
            } else {
                format!(r#"{{"error":"{error}"}}"#)
            };
            let mut fake = Fake::answering(&[(status, &body)]);
            let refused = start_with(&mut fake).unwrap_err();
            assert_eq!(refused.sentence(), sentence, "{body}");
            assert!(
                !refused.sentence().contains('{') && !refused.sentence().contains('_'),
                "{body}"
            );
            let mut fake = Fake::answering(&[(status, &body)]);
            assert_eq!(
                resend(&mut fake, "http://127.0.0.1:9", "r1", "s", None)
                    .unwrap_err()
                    .sentence(),
                sentence
            );
        }
        // The server's own sentence wins.
        let mut fake = Fake::answering(&[(
            429,
            r#"{"error":"code_already_sent","message":"A sign-in code was already sent to this address. Use it, or try again in 5 minutes.","retry_after":300}"#,
        )]);
        let refused = start_with(&mut fake).unwrap_err();
        assert_eq!(refused.code(), "code_already_sent");
        assert_eq!(
            refused.sentence(),
            "A sign-in code was already sent to this address. You can ask for a new one in 5 minutes."
        );
        // On resend and verify too, an answer that is not the contract's says sign-in is not available right now.
        for body in [r#"{"error":"something_new"}"#, "<html>oops</html>", ""] {
            let mut fake = Fake::answering(&[(500, body)]);
            let sentence = resend(&mut fake, "http://127.0.0.1:9", "r1", "s", None)
                .unwrap_err()
                .sentence();
            assert_eq!(
                sentence, "Sign-in is not available right now. Try again in a few minutes.",
                "{body}"
            );
        }
        let mut down = Fake::answering(&[]);
        assert_eq!(
            start_with(&mut down).unwrap_err().sentence(),
            "No connection. Check your internet and try again."
        );
    }

    /// A challenge page, a proxy's page or a redirect on any of the three paths is "not available right now"; an HTML
    /// 429 (the outer limit in front of the server) is "too many tries"; the server's own JSON 429 keeps its sentence.
    #[test]
    fn html_answers_are_not_available_and_an_outer_429_is_too_many_tries() {
        let challenge = "<!doctype html><title>Just a moment...</title>";
        for (status, body) in [
            (200, challenge),
            (403, challenge),
            (503, challenge),
            (302, ""),
        ] {
            let mut fake = Fake::answering(&[(status, body), (status, body), (status, body)]);
            assert_eq!(
                start_with(&mut fake),
                Err(EmailError::NotOffered),
                "start {status}"
            );
            assert_eq!(
                resend(&mut fake, "http://127.0.0.1:9", "r1", "s", None),
                Err(EmailError::NotOffered),
                "resend {status}"
            );
            assert_eq!(
                verify(&mut fake, "http://127.0.0.1:9", "r1", "s", "123456"),
                Err(EmailError::NotOffered),
                "verify {status}"
            );
        }
        for body in [challenge, "", "Too Many Requests"] {
            let mut fake = Fake::answering(&[(429, body), (429, body), (429, body)]);
            assert_eq!(start_with(&mut fake), Err(EmailError::Throttled));
            assert_eq!(
                resend(&mut fake, "http://127.0.0.1:9", "r1", "s", None),
                Err(EmailError::Throttled)
            );
            assert_eq!(
                verify(&mut fake, "http://127.0.0.1:9", "r1", "s", "1"),
                Err(EmailError::Throttled)
            );
            assert_eq!(fake.sent.len(), 3, "one request each, never tried again");
        }
        assert_eq!(
            EmailError::Throttled.sentence(),
            "Too many tries. Wait a moment and try again."
        );
        let mut fake =
            Fake::answering(&[(429, r#"{"error":"too_many_requests","retry_after":600}"#)]);
        assert_eq!(
            start_with(&mut fake).unwrap_err().sentence(),
            "Too many sign-in codes were asked for. Try again in 10 minutes."
        );
    }

    /// The code check's own answers: the network's limit on checks (with its wait, rounded up to minutes, "in a minute" under
    /// 90 seconds; the server's message, which may name another limit, is not said) and busy, whose sentence is the same
    /// on every page.
    #[test]
    fn a_throttled_or_busy_check_says_so_plainly() {
        for (retry, words) in [
            (Some(3600), "in 60 minutes"),
            (Some(90), "in 2 minutes"),
            (Some(89), "in a minute"),
            (None, "in a minute"),
        ] {
            let mut body = json!({"error":"too_many_requests","message":"Too many sign-in codes were sent to this address."});
            if let Some(s) = retry {
                body["retry_after"] = json!(s);
            }
            let mut fake = Fake::answering(&[(429, &body.to_string())]);
            let refused = verify(&mut fake, "http://127.0.0.1:9", "r1", "s", "123456").unwrap_err();
            assert_eq!(refused.code(), "too_many_requests");
            assert_eq!(
                refused.verify_sentence(),
                format!("Too many sign-in codes are being checked right now. Try again {words}.")
            );
        }
        let mut fake =
            Fake::answering(&[(503, r#"{"error":"busy","message":"The server is busy."}"#)]);
        let busy = verify(&mut fake, "http://127.0.0.1:9", "r1", "s", "123456").unwrap_err();
        assert_eq!(
            busy.code(),
            "busy",
            "the contract's, not taken for not offered"
        );
        assert_eq!(
            busy.verify_sentence(),
            "Sign-in is busy right now. Try again in a moment."
        );
        assert_eq!(busy.sentence(), busy.verify_sentence());
    }

    #[test]
    fn too_many_codes_says_the_wait_in_minutes_rounded_up() {
        for (retry, words) in [
            (Some(3600), "60 minutes"),
            (Some(61), "2 minutes"),
            (Some(60), "1 minute"),
            (Some(5), "1 minute"),
            (None, "1 minute"),
        ] {
            assert_eq!(
                local_sentence("too_many_requests", retry, None),
                format!("Too many sign-in codes were asked for. Try again in {words}.")
            );
        }
    }

    #[test]
    fn verify_sends_this_requests_id_and_secret_and_reads_the_token_answer() {
        let mut fake = Fake::answering(&[(
            200,
            r#"{"access_token":"a","refresh_token":"r","token_type":"Bearer","expires_in":900}"#,
        )]);
        let tokens = verify(&mut fake, "http://127.0.0.1:9", "r1", "secret", "246810").unwrap();
        assert_eq!(
            tokens,
            TokenSet {
                access_token: "a".into(),
                refresh_token: "r".into(),
                expires_in: 900
            }
        );
        assert_eq!(
            fake.sent[0].1,
            json!({"request_id":"r1","secret":"secret","code":"246810"})
        );
        assert_eq!(fake.sent[0].0, "http://127.0.0.1:9/auth/email/verify");
    }

    #[test]
    fn every_verify_error_has_a_plain_sentence() {
        let cases = [
            (
                r#"{"error":"invalid_code","attempts_left":3}"#,
                "invalid_code",
                "That code is not right. 3 tries left.",
            ),
            (
                r#"{"error":"invalid_code","attempts_left":1}"#,
                "invalid_code",
                "That code is not right. 1 try left.",
            ),
            // The contract's refusals carry a message; the count is said all the same.
            (
                r#"{"error":"invalid_code","message":"That code is not right.","attempts_left":4}"#,
                "invalid_code",
                "That code is not right. 4 tries left.",
            ),
            (
                r#"{"error":"invalid_code"}"#,
                "invalid_code",
                "That code is not right. Check the email and try again.",
            ),
            (
                r#"{"error":"code_expired"}"#,
                "code_expired",
                "This code has expired. Send a new code to try again.",
            ),
            (
                r#"{"error":"too_many_attempts"}"#,
                "too_many_attempts",
                "Too many wrong codes. Send a new code to try again.",
            ),
            (
                r#"{"error":"link_used"}"#,
                "link_used",
                "That sign-in link was already used. Start again.",
            ),
            (
                r#"{"error":"invalid_request"}"#,
                "invalid_request",
                "This sign-in request has ended. Start again.",
            ),
            (
                r#"{"error":"install_mismatch"}"#,
                "install_mismatch",
                "This code was requested on another device. Start again here.",
            ),
            (
                r#"{"error":"account_banned"}"#,
                "account_banned",
                "This Buwiz account is closed. Write to support@buwiz.com.",
            ),
        ];
        for (body, code, sentence) in cases {
            let status = if code == "account_banned" { 403 } else { 400 };
            let mut fake = Fake::answering(&[(status, body)]);
            let refused = verify(&mut fake, "http://127.0.0.1:9", "r1", "s", "000000").unwrap_err();
            assert_eq!(
                (refused.code(), refused.sentence().as_str()),
                (code, sentence),
                "{body}"
            );
        }
        let mut fake = Fake::answering(&[(400, r#"{"error":"invalid_code","attempts_left":2}"#)]);
        match verify(&mut fake, "http://127.0.0.1:9", "r1", "s", "000000").unwrap_err() {
            EmailError::Refused { attempts_left, .. } => assert_eq!(attempts_left, Some(2)),
            other => panic!("{other:?}"),
        }
    }

    /// Against the in-repo mock: a code from another request (another device's mail) is `invalid_code` here.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn against_the_mock_a_code_from_another_request_is_refused_and_its_own_signs_in() {
        use crate::mock_account::{MockAccount, MockOptions};
        let server =
            MockAccount::start_with("127.0.0.1:0".parse().unwrap(), MockOptions::default())
                .await
                .unwrap();
        let url = server.url();
        tokio::task::spawn_blocking(move || {
            let mut http = crate::cloud_account::BlockingHttp::new();
            let (mine, theirs) = (new_secret(), new_secret());
            let here = start(
                &mut http,
                &url,
                "here@example.test",
                Intent::SignIn,
                &mine,
                "macos",
                "install-id-of-this-test-0001",
                None,
            )
            .unwrap();
            let _there = start(
                &mut http,
                &url,
                "there@example.test",
                Intent::SignIn,
                &theirs,
                "ios",
                "install-id-of-another-0002",
                None,
            )
            .unwrap();
            let (code_here, code_there) = (
                server.code_for("here@example.test").unwrap(),
                server.code_for("there@example.test").unwrap(),
            );
            assert_ne!(code_here, code_there, "each request has its own code");
            let refused =
                verify(&mut http, &url, &here.request_id, &mine, &code_there).unwrap_err();
            assert_eq!(refused.code(), "invalid_code");
            assert!(refused.sentence().starts_with("That code is not right."));
            // This request's id with another secret: not this app's request.
            assert_eq!(
                verify(&mut http, &url, &here.request_id, &theirs, &code_here)
                    .unwrap_err()
                    .code(),
                "invalid_request"
            );
            let tokens = verify(&mut http, &url, &here.request_id, &mine, &code_here).unwrap();
            assert!(!tokens.refresh_token.is_empty());
            assert_eq!(
                resend(&mut http, &url, &here.request_id, &mine, None)
                    .unwrap_err()
                    .code(),
                "invalid_request"
            );
        })
        .await
        .unwrap();
    }

    fn pending() -> PendingEmail {
        PendingEmail::from_answer(
            "http://127.0.0.1:9",
            "filer@example.test",
            Intent::SignIn,
            "c2VjcmV0".into(),
            &EmailStarted {
                request_id: "r1".into(),
                expires_in: 300,
                resend_after: 300,
                code_length: 6,
            },
            1_000,
        )
    }

    #[test]
    fn the_pending_request_keeps_its_times_and_never_shows_its_secret() {
        let p = pending();
        assert_eq!((p.expires_at_unix, p.resend_at_unix), (1_300, 1_300));
        assert!(!p.expired(1_299));
        assert!(p.expired(1_300));
        let renewed = p.renewed(
            &EmailStarted {
                request_id: "r1".into(),
                expires_in: 300,
                resend_after: 280,
                code_length: 6,
            },
            1_400,
        );
        assert_eq!(
            (renewed.expires_at_unix, renewed.resend_at_unix),
            (1_700, 1_680)
        );
        let shown = format!("{p:?}");
        assert!(
            !shown.contains("c2VjcmV0") && !shown.contains("filer@") && !shown.contains("r1"),
            "{shown}"
        );
    }

    #[test]
    fn with_the_keychain_off_the_pending_request_is_a_private_file_until_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(PENDING_FILE_NAME);
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                (ENV_PENDING_FILE, Some(path.to_str().unwrap())),
            ],
            || {
                assert_eq!(load_pending(), None);
                save_pending(&pending()).unwrap();
                assert_eq!(load_pending(), Some(pending()));
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    assert_eq!(
                        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                        0o600
                    );
                }
                clear_pending();
                assert_eq!(load_pending(), None);
                assert!(!path.exists());
                std::fs::write(&path, "{broken").unwrap();
                assert_eq!(load_pending(), None, "a damaged file is no request");
            },
        );
    }
}
