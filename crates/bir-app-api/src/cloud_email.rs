//! Host verbs of the email-and-code sign-in (bir-core `email_sign_in`): start with an address, check the six-digit
//! code, Resend once the code has expired, end the request (Change email, the browser link, sign-out), and the pending
//! request after a relaunch. The page never sees the request id or the secret; the tokens are saved exactly as the
//! browser's device flow saves them ([`crate::cloud_account::save_signed_in`]).

use std::sync::Mutex;

use bir_core::cloud_account::{self, BlockingHttp, Http};
use bir_core::email_sign_in::{self, EmailError, GuardFor, Intent, PendingEmail};
use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};

pub const DECLARATIONS: &str = r#"
export interface CloudEmailStep {
    step: "code" | "refused" | "restart";
    email: string;
    code_length: number;
    expires_in: number;
    resend_in: number;
    message: string | null;
    retry_after: number | null;
}
export interface CloudEmailCheck {
    outcome: "approved" | "wrong" | "expired" | "too-many" | "wait" | "restart" | "error";
    message: string | null;
    attempts_left: number | null;
    status: CloudAccountStatus;
}
export function cloud_account_email_start(email: string): Promise<CloudEmailStep>;
export function cloud_account_email_verify(code: string): Promise<CloudEmailCheck>;
export function cloud_account_email_resend(): Promise<CloudEmailStep>;
export function cloud_account_email_cancel(): null;
export function cloud_account_email_pending(): CloudEmailStep | null;
"#;

/// One step of the email sign-in at a time: the pending request is read and written under it.
static STEP: Mutex<()> = Mutex::new(());

fn step_lock() -> std::sync::MutexGuard<'static, ()> {
    STEP.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn register(module: HostModule) -> HostModule {
    module
        .async_function("cloud_account_email_start", email_start)
        .async_function("cloud_account_email_verify", email_verify)
        .async_function("cloud_account_email_resend", email_resend)
        .function("cloud_account_email_cancel", |_| {
            cancel();
            Ok(HostValue::Null)
        })
        .function("cloud_account_email_pending", |_| {
            pending_now(cloud_account::unix_now())
        })
}

fn email_start(
    arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let email = arguments.string(0)?.to_owned();
    crate::cloud_account::spawn_account("buwiz-email-start", move || {
        start_on_the_api(&mut BlockingHttp::new(), &email, cloud_account::unix_now())
    })
}

fn email_verify(
    arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let code = arguments.string(0)?.to_owned();
    crate::cloud_account::spawn_account("buwiz-email-verify", move || {
        verify_with(&mut BlockingHttp::new(), &code, cloud_account::unix_now())
    })
}

fn email_resend(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    crate::cloud_account::spawn_account("buwiz-email-resend", move || {
        resend_with(&mut BlockingHttp::new(), cloud_account::unix_now())
    })
}

/// Ends the pending request (sign-out; Change email keeps it). Nothing pending: nothing to do.
pub fn cancel() {
    let _one = step_lock();
    email_sign_in::clear_pending();
}

/// A trimmed address the server is asked about. The server decides what a valid one is (`invalid_email`).
fn clean_email(raw: &str) -> String {
    raw.trim().to_owned()
}

fn step_value(
    step: &str,
    pending: Option<&PendingEmail>,
    now: i64,
    error: Option<&EmailError>,
) -> HostResult {
    let retry_after = match error {
        Some(EmailError::Refused { retry_after, .. }) => *retry_after,
        _ => None,
    };
    let number =
        |value: Option<i64>| value.map_or(HostValue::Null, |n| HostValue::Number(n as f64));
    Ok(HostObject::new()
        .field("step", step)
        .field(
            "email",
            pending.map(|p| p.email.clone()).unwrap_or_default(),
        )
        .field(
            "code_length",
            HostValue::Number(f64::from(pending.map_or(6, |p| p.code_length))),
        )
        .field(
            "expires_in",
            HostValue::Number(pending.map_or(0, |p| (p.expires_at_unix - now).max(0)) as f64),
        )
        .field(
            "resend_in",
            HostValue::Number(pending.map_or(0, |p| (p.resend_at_unix - now).max(0)) as f64),
        )
        .field(
            "message",
            error.map_or(HostValue::Null, |e| HostValue::Str(e.sentence())),
        )
        .field("retry_after", number(retry_after))
        .into())
}

/// The start on the API host: the configured server (`configured_server`, `https://api.buwiz.com` in a release
/// build), never the account site (`configured_account`), which answers these paths with a challenge page. Resend and
/// verify go to the server the pending request was started on.
pub(crate) fn start_on_the_api(http: &mut dyn Http, email: &str, now: i64) -> HostResult {
    let server = crate::cloud_account::resolve()?;
    start_with(http, &server.url, email, now)
}

/// `cloud_account_email_start`: a new request for `email`; once the server accepted it, it replaces any earlier one. A
/// server that does not offer it, a refusal or no connection: `refused` with the sentence, and the earlier request is
/// kept (a `code_already_sent` for its own address leaves its code usable). The page stays; it never switches to the
/// browser.
pub(crate) fn start_with(
    http: &mut dyn Http,
    server_url: &str,
    email: &str,
    now: i64,
) -> HostResult {
    let _one = step_lock();
    let email = clean_email(email);
    let secret = email_sign_in::new_secret();
    let answer = email_sign_in::start(
        http,
        server_url,
        &email,
        Intent::SignIn,
        &secret,
        bir_core::install_id::platform(),
        &bir_core::install_id::install_id(),
        email_sign_in::guard_for(GuardFor::Start),
    );
    match answer {
        Ok(answer) => {
            let pending =
                PendingEmail::from_answer(server_url, &email, Intent::SignIn, secret, &answer, now);
            email_sign_in::save_pending(&pending).map_err(HostError::new)?;
            step_value("code", Some(&pending), now, None)
        }
        Err(error) => step_value("refused", None, now, Some(&error)),
    }
}

/// `cloud_account_email_resend`: a new code for the same request, once the old one expired.
pub(crate) fn resend_with(http: &mut dyn Http, now: i64) -> HostResult {
    let _one = step_lock();
    let Some(pending) = email_sign_in::load_pending() else {
        return Err(HostError::new(
            "No sign-in code is waiting. Enter your email again.",
        ));
    };
    match email_sign_in::resend(
        http,
        &pending.server_url,
        &pending.request_id,
        &pending.secret,
        email_sign_in::guard_for(GuardFor::Resend),
    ) {
        Ok(answer) => {
            let renewed = pending.renewed(&answer, now);
            email_sign_in::save_pending(&renewed).map_err(HostError::new)?;
            step_value("code", Some(&renewed), now, None)
        }
        Err(error) => {
            let mut kept = pending.clone();
            // Too early after all (the server's clock): Resend waits as long as the server says.
            if let EmailError::Refused {
                retry_after: Some(wait),
                ..
            } = &error
            {
                kept.resend_at_unix = now.saturating_add(*wait);
                let _ = email_sign_in::save_pending(&kept);
            }
            // An ended request (an hour after the start, or its mail failed): back to the email page.
            if ends_the_request(error.code()) || error.code() == "mail_failed" {
                email_sign_in::clear_pending();
                return step_value("restart", None, now, Some(&error));
            }
            step_value("refused", Some(&kept), now, Some(&error))
        }
    }
}

/// The verify answers after which the request cannot go on: back to the email page.
fn ends_the_request(code: &str) -> bool {
    matches!(
        code,
        "invalid_request" | "link_used" | "install_mismatch" | "account_banned"
    )
}

fn check_value(outcome: &str, error: Option<&EmailError>) -> HostResult {
    let attempts = match error {
        Some(EmailError::Refused {
            attempts_left: Some(n),
            ..
        }) => HostValue::Number(*n as f64),
        _ => HostValue::Null,
    };
    Ok(HostObject::new()
        .field("outcome", outcome)
        .field(
            "message",
            error.map_or(HostValue::Null, |e| HostValue::Str(e.verify_sentence())),
        )
        .field("attempts_left", attempts)
        .field("status", crate::cloud_account::status_now()?)
        .into())
}

/// `cloud_account_email_verify`: the code for the pending request. Approved: the tokens are saved as the browser's
/// device flow saves them and the request is gone. Wrong, expired, too many: the request stays (Resend). `wait`: the
/// network's limit on code checks (`too_many_requests`, with its wait) or a busy server (`busy`): no try was spent, and
/// the same request takes the code again later.
pub(crate) fn verify_with(http: &mut dyn Http, code: &str, now: i64) -> HostResult {
    let _one = step_lock();
    let Some(pending) = email_sign_in::load_pending() else {
        return check_value(
            "restart",
            Some(&EmailError::Refused {
                code: "invalid_request".into(),
                message: String::new(),
                retry_after: None,
                attempts_left: None,
            }),
        );
    };
    let code: String = code.chars().filter(char::is_ascii_digit).collect();
    match email_sign_in::verify(
        http,
        &pending.server_url,
        &pending.request_id,
        &pending.secret,
        &code,
    ) {
        Ok(tokens) => {
            email_sign_in::clear_pending();
            crate::cloud_account::save_signed_in(http, &pending.server_url, tokens, now)?;
            check_value("approved", None)
        }
        Err(error) => {
            let outcome = match error.code() {
                "invalid_code" => "wrong",
                "code_expired" => "expired",
                "too_many_attempts" => "too-many",
                "too_many_requests" | "busy" => "wait",
                code if ends_the_request(code) => {
                    email_sign_in::clear_pending();
                    "restart"
                }
                _ => "error",
            };
            check_value(outcome, Some(&error))
        }
    }
}

/// `cloud_account_email_pending`: the request kept from before a relaunch, with its countdowns, or null. One that
/// has expired, or is for another server, is cleared and not shown.
pub(crate) fn pending_now(now: i64) -> HostResult {
    let _one = step_lock();
    let Some(pending) = email_sign_in::load_pending() else {
        return Ok(HostValue::Null);
    };
    let server = crate::cloud_account::resolve()
        .ok()
        .map(|server| server.url);
    if pending.expired(now) || server.as_deref() != Some(pending.server_url.as_str()) {
        email_sign_in::clear_pending();
        return Ok(HostValue::Null);
    }
    step_value("code", Some(&pending), now, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::cloud_account::{CloudError, HttpRequest, HttpResponse};

    /// A fake Buwiz server: answers by path, in order; what it was sent.
    #[derive(Default)]
    struct Fake {
        answers: Vec<(&'static str, u16, String)>,
        sent: Vec<(String, serde_json::Value)>,
        urls: Vec<String>,
    }

    impl Fake {
        fn answer(mut self, path: &'static str, status: u16, body: &str) -> Self {
            self.answers.push((path, status, body.to_owned()));
            self
        }
    }

    impl Http for Fake {
        fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
            self.urls.push(request.url.clone());
            let path = request.url.trim_start_matches(SERVER).to_owned();
            self.sent.push((
                path.clone(),
                serde_json::from_str(&request.body).unwrap_or_default(),
            ));
            let i = self
                .answers
                .iter()
                .position(|(p, ..)| *p == path)
                .ok_or_else(|| CloudError::Network("no answer".into()))?;
            let (_, status, body) = self.answers.remove(i);
            Ok(HttpResponse { status, body })
        }
    }

    const SERVER: &str = "http://127.0.0.1:9";
    const STARTED: &str =
        r#"{"request_id":"r1","expires_in":300,"resend_after":300,"code_length":6}"#;
    const TOKENS: &str = r#"{"access_token":"a1","refresh_token":"f1","expires_in":900}"#;
    const ME: &str = r#"{"user_id":"u1","email":"filer@example.test"}"#;

    fn json(value: HostResult) -> serde_json::Value {
        crate::bir_module::to_json(&value.expect("an answer"))
    }

    /// Each test works in its own scratch folder (the pending request and the saved sign-in beside its database).
    fn scratch<R>(body: impl FnOnce() -> R) -> R {
        let dir = tempfile::tempdir().unwrap();
        bir_core::db::set_database_path_override(Some(dir.path().join("bir_data.db")));
        let result = temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                ("BUWIZ_SERVER_URL", Some(SERVER)),
                (
                    bir_core::email_sign_in::ENV_PENDING_FILE,
                    Some(dir.path().join("pending.json").to_str().unwrap()),
                ),
                (
                    cloud_account::ENV_ACCOUNT_FILE,
                    Some(dir.path().join("account.json").to_str().unwrap()),
                ),
            ],
            body,
        );
        bir_core::db::set_database_path_override(None);
        result
    }

    #[test]
    fn start_keeps_the_request_and_answers_the_countdowns_from_the_server() {
        scratch(|| {
            let mut fake = Fake::default().answer(
                "/auth/email/start",
                200,
                r#"{"request_id":"r1","expires_in":240,"resend_after":200,"code_length":6}"#,
            );
            let step = json(start_with(
                &mut fake,
                SERVER,
                "  filer@example.test ",
                1_000,
            ));
            assert_eq!(step["step"], "code");
            assert_eq!(step["email"], "filer@example.test");
            assert_eq!(
                (step["expires_in"].as_f64(), step["resend_in"].as_f64()),
                (Some(240.0), Some(200.0))
            );
            assert!(
                !step.to_string().contains("r1"),
                "the page never sees the request id"
            );
            let sent = &fake.sent[0].1;
            assert_eq!(sent["intent"], "sign_in");
            assert!(bir_core::install_id::valid(
                sent["install_id"].as_str().unwrap()
            ));
            assert!(["macos", "linux", "windows"].contains(&sent["platform"].as_str().unwrap()));
            let kept = email_sign_in::load_pending().expect("kept");
            assert_eq!((kept.expires_at_unix, kept.resend_at_unix), (1_240, 1_200));
            assert_eq!(
                sent["secret_hash"],
                email_sign_in::secret_hash(&kept.secret)
            );
            // After a relaunch, 100 seconds later: the code page with what is left.
            let restored = json(pending_now(1_100));
            assert_eq!(
                (
                    restored["expires_in"].as_f64(),
                    restored["resend_in"].as_f64()
                ),
                (Some(140.0), Some(100.0))
            );
            assert_eq!(restored["email"], "filer@example.test");
            // Expired by the next launch: cleared, not shown.
            assert_eq!(json(pending_now(1_240)), serde_json::Value::Null);
            assert!(email_sign_in::load_pending().is_none());
        });
    }

    #[test]
    fn not_offered_or_refused_stays_on_the_email_page_and_keeps_nothing() {
        scratch(|| {
            for (status, body) in [(404, ""), (200, "<!doctype html>"), (302, "")] {
                let mut fake = Fake::default().answer("/auth/email/start", status, body);
                let step = json(start_with(&mut fake, SERVER, "filer@example.test", 1_000));
                assert_eq!(step["step"], "refused", "never the browser by itself");
                assert_eq!(
                    step["message"],
                    "Sign-in is not available right now. Try again in a few minutes."
                );
            }
            for code in ["turnstile_required", "turnstile_failed"] {
                let body = format!(r#"{{"error":"{code}"}}"#);
                let mut fake = Fake::default().answer("/auth/email/start", 403, &body);
                let step = json(start_with(&mut fake, SERVER, "filer@example.test", 1_000));
                assert_eq!(
                    (step["step"].as_str(), step["message"].as_str()),
                    (
                        Some("refused"),
                        Some("Sign-in could not be checked on this device. Try again.")
                    )
                );
            }
            assert!(email_sign_in::load_pending().is_none());
            let mut fake = Fake::default().answer(
                "/auth/email/start",
                400,
                r#"{"error":"invalid_email","message":"That email address does not look right."}"#,
            );
            let step = json(start_with(&mut fake, SERVER, "nope", 1_000));
            assert_eq!(
                (step["step"].as_str(), step["message"].as_str()),
                (
                    Some("refused"),
                    Some("That email address does not look right.")
                )
            );
            let mut fake = Fake::default().answer(
                "/auth/email/start",
                429,
                r#"{"error":"code_already_sent","retry_after":180}"#,
            );
            let step = json(start_with(&mut fake, SERVER, "filer@example.test", 1_000));
            assert_eq!(
                step["message"],
                "A sign-in code was already sent to this address. You can ask for a new one in 3 minutes."
            );
            assert_eq!(step["retry_after"], 180.0);
            let mut down = Fake::default();
            let step = json(start_with(&mut down, SERVER, "filer@example.test", 1_000));
            assert_eq!(
                step["message"],
                "No connection. Check your internet and try again."
            );
            assert!(email_sign_in::load_pending().is_none());
        });
    }

    #[test]
    fn wrong_expired_and_too_many_keep_the_request_and_an_ended_one_starts_over() {
        scratch(|| {
            let cases = [
                (
                    r#"{"error":"invalid_code","attempts_left":2}"#,
                    "wrong",
                    "That code is not right. 2 tries left.",
                    true,
                ),
                (
                    r#"{"error":"code_expired"}"#,
                    "expired",
                    "This code has expired. Send a new code to try again.",
                    true,
                ),
                (
                    r#"{"error":"too_many_attempts"}"#,
                    "too-many",
                    "Too many wrong codes. Send a new code to try again.",
                    true,
                ),
                (
                    r#"{"error":"invalid_request"}"#,
                    "restart",
                    "This sign-in request has ended. Start again.",
                    false,
                ),
                (
                    r#"{"error":"link_used"}"#,
                    "restart",
                    "That sign-in link was already used. Start again.",
                    false,
                ),
                (
                    r#"{"error":"install_mismatch"}"#,
                    "restart",
                    "This code was requested on another device. Start again here.",
                    false,
                ),
                (
                    r#"{"error":"account_banned","message":"This account is closed."}"#,
                    "restart",
                    "This account is closed.",
                    false,
                ),
            ];
            for (body, outcome, sentence, kept) in cases {
                let mut fake = Fake::default()
                    .answer("/auth/email/start", 200, STARTED)
                    .answer("/auth/email/verify", 400, body);
                start_with(&mut fake, SERVER, "filer@example.test", 1_000).unwrap();
                let check = json(verify_with(&mut fake, "123 456", 1_010));
                assert_eq!(
                    (check["outcome"].as_str(), check["message"].as_str()),
                    (Some(outcome), Some(sentence)),
                    "{body}"
                );
                assert_eq!(email_sign_in::load_pending().is_some(), kept, "{body}");
                assert_eq!(fake.sent[1].1["code"], "123456", "digits only");
                assert_eq!(check["status"]["signed_in"], false);
            }
            let mut fake = Fake::default().answer("/auth/email/start", 200, STARTED);
            start_with(&mut fake, SERVER, "filer@example.test", 1_000).unwrap();
            let check = json(verify_with(&mut Fake::default(), "123456", 1_010));
            assert_eq!(
                check["outcome"], "error",
                "unreachable: try again, the request stays"
            );
            assert!(email_sign_in::load_pending().is_some());
            let wrong = json(verify_with(
                &mut Fake::default().answer(
                    "/auth/email/verify",
                    400,
                    r#"{"error":"invalid_code","attempts_left":4}"#,
                ),
                "1",
                1_010,
            ));
            assert_eq!(wrong["attempts_left"], 4.0);
        });
    }

    #[test]
    fn the_right_code_saves_the_sign_in_and_ends_the_request() {
        scratch(|| {
            let mut fake = Fake::default()
                .answer("/auth/email/start", 200, STARTED)
                .answer("/auth/email/verify", 200, TOKENS)
                .answer("/me", 200, ME);
            start_with(&mut fake, SERVER, "filer@example.test", 1_000).unwrap();
            let check = json(verify_with(&mut fake, "246810", 1_010));
            assert_eq!(check["outcome"], "approved");
            assert_eq!(check["status"]["signed_in"], true);
            assert_eq!(check["status"]["email"], "filer@example.test");
            assert!(email_sign_in::load_pending().is_none());
            let saved = cloud_account::load_stored_account().unwrap().unwrap();
            assert_eq!(
                (saved.refresh_token.as_str(), saved.access_expires_at_unix),
                ("f1", 1_910)
            );
            let again = json(verify_with(&mut fake, "246810", 1_011));
            assert_eq!(again["outcome"], "restart");
            cloud_account::clear_stored_account();
        });
    }

    #[test]
    fn resend_renews_the_countdowns_and_a_too_early_one_waits_as_the_server_says() {
        scratch(|| {
            assert!(
                resend_with(&mut Fake::default(), 1_000).is_err(),
                "nothing pending"
            );
            let mut fake = Fake::default()
                .answer("/auth/email/start", 200, STARTED)
                .answer(
                    "/auth/email/resend",
                    429,
                    r#"{"error":"too_many_requests","retry_after":60}"#,
                )
                .answer("/auth/email/resend", 200, STARTED);
            start_with(&mut fake, SERVER, "filer@example.test", 1_000).unwrap();
            let early = json(resend_with(&mut fake, 1_240));
            assert_eq!(early["step"], "refused");
            assert_eq!(early["resend_in"], 60.0);
            assert_eq!(early["expires_in"], 60.0);
            assert_eq!(email_sign_in::load_pending().unwrap().resend_at_unix, 1_300);
            let renewed = json(resend_with(&mut fake, 1_300));
            assert_eq!(
                (
                    renewed["step"].as_str(),
                    renewed["expires_in"].as_f64(),
                    renewed["resend_in"].as_f64()
                ),
                (Some("code"), Some(300.0), Some(300.0))
            );
            assert_eq!(fake.sent.last().unwrap().1["request_id"], "r1");
            assert!(fake.sent.last().unwrap().1.get("turnstile").is_none());
            // Its mail failed, or an hour has passed: the request ended, back to the email page.
            for (status, body) in [
                (502, r#"{"error":"mail_failed"}"#),
                (400, r#"{"error":"invalid_request"}"#),
            ] {
                let mut fake = Fake::default()
                    .answer("/auth/email/start", 200, STARTED)
                    .answer("/auth/email/resend", status, body);
                start_with(&mut fake, SERVER, "filer@example.test", 2_000).unwrap();
                assert_eq!(
                    json(resend_with(&mut fake, 2_300))["step"],
                    "restart",
                    "{body}"
                );
                assert!(email_sign_in::load_pending().is_none());
            }
            let mut fake = Fake::default().answer("/auth/email/start", 200, STARTED);
            start_with(&mut fake, SERVER, "filer@example.test", 3_000).unwrap();
            cancel();
            assert!(
                email_sign_in::load_pending().is_none(),
                "Change email ends it"
            );
        });
    }

    #[test]
    fn a_request_for_another_server_is_not_restored() {
        scratch(|| {
            let mut fake = Fake::default().answer("/auth/email/start", 200, STARTED);
            start_with(
                &mut fake,
                "http://127.0.0.1:10",
                "filer@example.test",
                1_000,
            )
            .unwrap();
            assert_eq!(json(pending_now(1_001)), serde_json::Value::Null);
            assert!(email_sign_in::load_pending().is_none());
        });
    }

    /// The three calls go to the API host, never the account site, even when the two differ.
    #[test]
    fn the_three_calls_go_to_the_api_host_and_never_the_account_site() {
        scratch(|| {
            temp_env::with_var("BUWIZ_ACCOUNT_URL", Some("http://127.0.0.1:7"), || {
                let mut fake = Fake::default()
                    .answer("/auth/email/start", 200, STARTED)
                    .answer("/auth/email/resend", 200, STARTED)
                    .answer(
                        "/auth/email/verify",
                        400,
                        r#"{"error":"invalid_code","attempts_left":4}"#,
                    );
                start_on_the_api(&mut fake, "filer@example.test", 1_000).unwrap();
                resend_with(&mut fake, 1_400).unwrap();
                verify_with(&mut fake, "123456", 1_401).unwrap();
                let urls: Vec<&str> = fake.urls.iter().map(String::as_str).collect();
                assert_eq!(
                    urls,
                    [
                        "http://127.0.0.1:9/auth/email/start",
                        "http://127.0.0.1:9/auth/email/resend",
                        "http://127.0.0.1:9/auth/email/verify",
                    ]
                );
            });
        });
    }

    /// A challenge page or an outer 429 leaves the person where they are: the request is kept, nothing ends.
    #[test]
    fn an_html_answer_keeps_the_request_and_says_not_available_or_too_many_tries() {
        scratch(|| {
            let mut fake = Fake::default().answer("/auth/email/start", 200, STARTED);
            start_with(&mut fake, SERVER, "filer@example.test", 1_000).unwrap();
            for (status, sentence) in [
                (
                    200,
                    "Sign-in is not available right now. Try again in a few minutes.",
                ),
                (429, "Too many tries. Wait a moment and try again."),
            ] {
                let mut html = Fake::default()
                    .answer("/auth/email/verify", status, "<!doctype html>")
                    .answer("/auth/email/resend", status, "<!doctype html>");
                let check = json(verify_with(&mut html, "123456", 1_010));
                assert_eq!(
                    (check["outcome"].as_str(), check["message"].as_str()),
                    (Some("error"), Some(sentence))
                );
                let step = json(resend_with(&mut html, 1_400));
                assert_eq!(
                    (step["step"].as_str(), step["message"].as_str()),
                    (Some("refused"), Some(sentence))
                );
                assert!(
                    email_sign_in::load_pending().is_some(),
                    "the request is kept"
                );
                assert_eq!(html.sent.len(), 2, "one request each");
            }
            let mut html = Fake::default().answer("/auth/email/start", 200, "<!doctype html>");
            let step = json(start_with(&mut html, SERVER, "filer@example.test", 2_000));
            assert_eq!(
                step["message"],
                "Sign-in is not available right now. Try again in a few minutes."
            );
        });
    }

    #[test]
    fn a_refused_start_keeps_the_request_before_it() {
        scratch(|| {
            let mut fake = Fake::default().answer("/auth/email/start", 200, STARTED);
            json(start_with(&mut fake, SERVER, "filer@example.test", 1_000));
            let before = email_sign_in::load_pending().expect("kept");
            let mut refused = Fake::default().answer(
                "/auth/email/start",
                429,
                r#"{"error":"code_already_sent","message":"A sign-in code was already sent to this address.","retry_after":240}"#,
            );
            let step = json(start_with(
                &mut refused,
                SERVER,
                "filer@example.test",
                1_060,
            ));
            assert_eq!(step["step"], "refused");
            // The wait is said even when the message leaves it out.
            assert_eq!(
                step["message"],
                "A sign-in code was already sent to this address. You can ask for a new one in 4 minutes."
            );
            assert_eq!(email_sign_in::load_pending(), Some(before));
        });
    }

    /// The in-repo mock account server (`bir_core::mock_account`, what `just mock-account` runs) on a runtime of its own,
    /// so the host's own calls (blocking HTTP) reach it as they reach a real server.
    fn against_the_mock<R>(
        options: bir_core::mock_account::MockOptions,
        body: impl FnOnce(&bir_core::mock_account::MockAccount, &str) -> R,
    ) -> R {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mock = runtime
            .block_on(bir_core::mock_account::MockAccount::start_with(
                "127.0.0.1:0".parse().unwrap(),
                options,
            ))
            .unwrap();
        let url = mock.url();
        scratch(|| temp_env::with_var("BUWIZ_SERVER_URL", Some(&url), || body(&mock, &url)))
    }

    #[test]
    fn against_the_mock_a_wrong_code_says_the_tries_left() {
        against_the_mock(Default::default(), |_, url| {
            let mut http = BlockingHttp::new();
            let now = cloud_account::unix_now();
            assert_eq!(
                json(start_with(&mut http, url, "filer@example.test", now))["step"],
                "code"
            );
            let check = json(verify_with(&mut http, "123456", now));
            assert_eq!(check["outcome"], "wrong");
            assert_eq!(check["message"], "That code is not right. 4 tries left.");
            assert_eq!(check["attempts_left"], 4.0);
            let check = json(verify_with(&mut http, "123457", now));
            assert_eq!(check["message"], "That code is not right. 3 tries left.");
        });
    }

    #[test]
    fn against_the_mock_a_second_start_sends_no_code_and_the_first_still_works() {
        against_the_mock(Default::default(), |mock, url| {
            let mut http = BlockingHttp::new();
            let now = cloud_account::unix_now();
            let first = json(start_with(&mut http, url, "filer@example.test", now));
            assert_eq!(first["step"], "code");
            let second = json(start_with(&mut http, url, " Filer@Example.test", now + 20));
            assert_eq!(second["step"], "refused");
            assert_eq!(
                second["message"],
                "A sign-in code was already sent to this address. You can ask for a new one in 5 minutes."
            );
            assert!(second["retry_after"].as_f64().unwrap() > 0.0);
            // One code mailed, its time not restarted, and it still signs in.
            assert_eq!(mock.codes_mailed("filer@example.test"), 1);
            assert_eq!(json(pending_now(now + 20))["expires_in"], 280.0);
            let check = json(verify_with(
                &mut http,
                bir_core::mock_account::DEFAULT_CODE,
                now + 30,
            ));
            assert_eq!(check["outcome"], "approved");
        });
    }

    /// An old request's code no longer works. Change email to another address and back, inside the first code's life,
    /// starts nothing new (`code_already_sent`: the server holds this install for that address); once the first code
    /// has expired, Continue with the same address starts a new request with its own code. The first request's code,
    /// typed now, is a wrong try of the new request (`invalid_code`, rule A: a code works only with its own request);
    /// the new code signs in.
    #[test]
    fn against_the_mock_an_old_request_s_code_is_refused_and_the_new_one_signs_in() {
        let options = bir_core::mock_account::MockOptions {
            code_ttl: std::time::Duration::from_secs(1),
            resend_wait: std::time::Duration::from_secs(1),
            ..Default::default()
        };
        against_the_mock(options, |mock, url| {
            let mut http = BlockingHttp::new();
            let now = cloud_account::unix_now();
            assert_eq!(
                json(start_with(&mut http, url, "juana@example.test", now))["step"],
                "code"
            );
            let first = mock.code_for("juana@example.test").unwrap();
            // Change email: another address, then back while the first code lives.
            assert_eq!(
                json(start_with(&mut http, url, "pedro@example.test", now))["step"],
                "code"
            );
            let back = json(start_with(&mut http, url, "juana@example.test", now));
            assert_eq!(back["step"], "refused", "{back}");
            std::thread::sleep(std::time::Duration::from_millis(1100));
            let now = cloud_account::unix_now();
            assert_eq!(
                json(start_with(&mut http, url, "juana@example.test", now))["step"],
                "code"
            );
            let second = mock.code_for("juana@example.test").unwrap();
            assert_ne!(first, second);
            let old = json(verify_with(&mut http, &first, now));
            assert_eq!(old["outcome"], "wrong");
            assert_eq!(old["message"], "That code is not right. 4 tries left.");
            let new = json(verify_with(&mut http, &second, now));
            assert_eq!(new["outcome"], "approved");
            assert_eq!(new["status"]["email"], "juana@example.test");
        });
    }

    /// The network's limits and a busy server, through the host against the mock. A start or resend over the network's
    /// limit says a sentence true for either limit; a code check over its own limit, or met by a busy server, is `wait`
    /// with its sentence, spends no try, and the same request then takes the right code.
    #[test]
    fn against_the_mock_the_network_limits_and_busy_spend_no_try() {
        let options = bir_core::mock_account::MockOptions {
            start_per_network: 1,
            verify_per_network: 2,
            ..Default::default()
        };
        against_the_mock(options, |mock, url| {
            let mut http = BlockingHttp::new();
            let now = cloud_account::unix_now();
            assert_eq!(
                json(start_with(&mut http, url, "juana@example.test", now))["step"],
                "code"
            );
            let code = mock.code_for("juana@example.test").unwrap();
            let refused = json(start_with(&mut http, url, "pedro@example.test", now));
            assert_eq!(refused["step"], "refused");
            assert_eq!(
                refused["message"],
                "Too many sign-in codes were asked for. Try again in 60 minutes."
            );
            assert_eq!(refused["retry_after"], 3600.0);
            // Two checks this hour: a wrong one (4 tries left), then the network's limit.
            assert_eq!(
                json(verify_with(&mut http, "000000", now))["attempts_left"],
                4.0
            );
            assert_eq!(
                json(verify_with(&mut http, "000001", now))["attempts_left"],
                3.0
            );
            let wait = json(verify_with(&mut http, &code, now));
            assert_eq!(wait["outcome"], "wait");
            assert_eq!(
                wait["message"],
                "Too many sign-in codes are being checked right now. Try again in 60 minutes."
            );
            assert_eq!(wait["attempts_left"], serde_json::Value::Null);
            assert!(
                email_sign_in::load_pending().is_some(),
                "the request is kept"
            );
        });
        let busy = bir_core::mock_account::MockOptions {
            busy_verify: true,
            ..Default::default()
        };
        against_the_mock(busy, |_, url| {
            let mut http = BlockingHttp::new();
            let now = cloud_account::unix_now();
            start_with(&mut http, url, "juana@example.test", now).unwrap();
            let check = json(verify_with(&mut http, "246810", now));
            assert_eq!(check["outcome"], "wait");
            assert_eq!(
                check["message"],
                "Sign-in is busy right now. Try again in a moment."
            );
            assert!(email_sign_in::load_pending().is_some());
        });
    }

    #[test]
    fn the_page_is_told_no_request_id_or_secret() {
        assert!(!DECLARATIONS.contains("request_id"));
        assert!(!DECLARATIONS.contains("secret"));
    }
}
