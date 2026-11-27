//! Sending a return to the BIR through the Buwiz relay.
//!
//! The client is `crates/bir-submit`: it seals the plain return for Buwiz's
//! sender, pipes it through this computer to the BIR's own hosts (only the
//! ones on Buwiz's signed trust list) and comes back with a signed receipt.
//! This module is the queue's side of it: one [`Submitter`] per process, with
//! the app's one sign-in ([`crate::account_session::AccountTokens`]), and the
//! translation of the client's answers into the queue's outcomes
//! ([`send_outcome`]).
//!
//! What the queue does with each outcome is in
//! [`crate::forms::queueable::NoReceipt`]. The rules that must hold here:
//! a return the BIR may have is never an outcome that lets the queue send it
//! again, and a hold is never counted as a failed try.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bir_submit::wire::{TinRefusal, is_final_reason};
use bir_submit::{SendError, Sent, SubmitConfig, Submitter};

use crate::transport::{StoreReport, TransportError};

/// The longest one send may take before its outcome is called unknown. The
/// relay pipe itself may run 9 minutes.
const SEND_LIMIT: Duration = Duration::from_secs(600);

/// How much of a refusal's sentence is kept on the return (its code is
/// always kept whole).
const REFUSAL_CHARS: usize = 300;

/// The process's submitter, with the server it was made for.
static SUBMITTER: Mutex<Option<(String, Arc<Submitter>)>> = Mutex::new(None);

/// An http(s) origin on this computer.
fn on_this_computer(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let host = parsed
        .host_str()
        .unwrap_or_default()
        .trim_start_matches('[')
        .trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Whether a build may send through the Buwiz server at `url`. A development
/// build sends only through a server on this computer: it can never file a
/// real return through production, whatever its environment says.
fn server_allowed(url: &str, development_build: bool) -> Result<(), TransportError> {
    if development_build && !on_this_computer(url) {
        return Err(TransportError::Config(
            "A development build sends only through a Buwiz server on this computer (BUWIZ_SERVER_URL=http://localhost:PORT)."
                .into(),
        ));
    }
    Ok(())
}

/// Whether a build may send on a trust list signed by `trust_keys`. A development build never trusts the key that
/// signs production's list (the one compiled in, which names the BIR's real hosts): it needs a test key in
/// `BUWIZ_TRUST_KEYS`, so even a server on this computer that serves production's list is refused.
fn trust_keys_allowed(
    trust_keys: &[String],
    development_build: bool,
) -> Result<(), TransportError> {
    let production = |key: &String| bir_submit::trust::TRUST_KEYS.contains(&key.as_str());
    if development_build && (trust_keys.is_empty() || trust_keys.iter().any(production)) {
        return Err(TransportError::Config(
            "A development build sends only on a trust list signed by a test key (BUWIZ_TRUST_KEYS), never on production's."
                .into(),
        ));
    }
    Ok(())
}

/// The trust list's pinned keys for this process: `BUWIZ_TRUST_KEYS` as [`crate::config::var`] reads it (the
/// process environment, or an Android development build's baked list), under `bir_submit`'s own rule: it replaces
/// the production key only in a debug or `lab` build.
fn trust_keys() -> Vec<String> {
    bir_submit::trust::pinned_keys_from(
        crate::config::var(bir_submit::trust::KEYS_ENV)
            .ok()
            .as_deref(),
    )
}

/// The Buwiz server this build may send through, or why it may not: the configured server, which for a
/// development build must be on this computer and trusted through a test key. No request is made.
pub fn server() -> Result<String, TransportError> {
    let server = crate::cloud_account::configured_server()
        .map_err(|error| TransportError::Config(error.to_string()))?;
    server_allowed(&server.url, cfg!(debug_assertions))?;
    trust_keys_allowed(&trust_keys(), cfg!(debug_assertions))?;
    Ok(server.url)
}

/// Where the device key and id live when the keychain is not used: the device folder, as the saved sign-in
/// is (`platform::device_dir`), whichever room is open. A scratch database (a development, test or agent run) gets
/// a scratch device, never a file in the app's real data folder.
pub(crate) fn secrets_dir() -> std::path::PathBuf {
    crate::platform::device_dir()
}

/// The client for the configured Buwiz server, made once per server.
pub fn submitter() -> Result<Arc<Submitter>, TransportError> {
    let server = crate::cloud_account::configured_server()
        .map_err(|error| TransportError::Config(error.to_string()))?;
    server_allowed(&server.url, cfg!(debug_assertions))?;
    let mut slot = SUBMITTER
        .lock()
        .map_err(|_| TransportError::Config("the relay client is unavailable".into()))?;
    if let Some((url, submitter)) = slot.as_ref()
        && *url == server.url
    {
        return Ok(submitter.clone());
    }
    let mut config = SubmitConfig::new(&server.url)
        .map_err(|error| TransportError::Config(error.to_string()))?;
    config.install_id = Some(crate::install_id::install_id());
    config.platform = Some(crate::install_id::platform().to_owned());
    config.trust_keys = trust_keys();
    trust_keys_allowed(&config.trust_keys, cfg!(debug_assertions))?;
    let store =
        bir_submit::secrets::platform_store(secrets_dir(), !crate::platform::keychain_disabled());
    let submitter = Arc::new(Submitter::new(
        config,
        store,
        Arc::new(crate::account_session::AccountTokens),
    ));
    submitter.set_client("buwiz-forms", env!("CARGO_PKG_VERSION"));
    *slot = Some((server.url, submitter.clone()));
    Ok(submitter)
}

/// Forget this device's key on the Buwiz server's side of things: a new
/// sign-in registers a new key. The account layer calls it on sign-in and
/// sign-out. Nothing to forget when the client cannot be made.
pub fn forget_device() {
    if let Ok(submitter) = submitter()
        && let Err(error) = submitter.forget_device()
    {
        tracing::warn!("could not forget the relay device key: {error}");
    }
}

/// What [`register_after_sign_in`] did. Never an error the sign-in sees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Registration {
    /// A practice account never sends, so it registers no device.
    Practice,
    /// No client for this build or server (a development build without a test trust key, say): the first send
    /// registers, or says why it cannot.
    NoClient(String),
    Registered,
    /// This sign-in already has its device (`409 device_registered`).
    AlreadyDone,
    /// It did not go through (offline, refused): logged; the first send registers as it always did.
    Failed(String),
}

/// Adds this device to the account right after a sign-in's tokens are saved (after [`forget_device`]): once, with the
/// install id and the platform. A practice account registers none. Blocks the calling thread for the one request, so the
/// host calls it off the sign-in's path; nothing here can fail the sign-in.
pub fn register_after_sign_in() -> Registration {
    let outcome = register_with(crate::transport::practice_account(), submitter);
    match &outcome {
        Registration::Practice => {
            tracing::info!("device registration: a practice account adds no device")
        }
        Registration::NoClient(why) => {
            tracing::info!("device registration: not now ({why}); the first send registers")
        }
        Registration::Registered => tracing::info!("device registration: this device was added"),
        Registration::AlreadyDone => {
            tracing::info!("device registration: already done for this sign-in")
        }
        Registration::Failed(why) => {
            tracing::warn!("device registration failed ({why}); the first send registers")
        }
    }
    outcome
}

fn register_with(
    practice: bool,
    client: impl FnOnce() -> Result<Arc<Submitter>, TransportError>,
) -> Registration {
    if practice {
        return Registration::Practice;
    }
    let submitter = match client() {
        Ok(submitter) => submitter,
        Err(error) => return Registration::NoClient(error.to_string()),
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return Registration::Failed(error.to_string()),
    };
    match runtime.block_on(submitter.register_device()) {
        Ok(bir_submit::Registered::New) => Registration::Registered,
        Ok(bir_submit::Registered::AlreadyDone) => Registration::AlreadyDone,
        Err(error) => Registration::Failed(error.to_string()),
    }
}

/// The saved sign-in is for the server the client talks to. A token is never offered to another server: signed
/// out, or signed in elsewhere, the return is held until the person signs in.
fn signed_in_to(server_url: &str) -> Result<(), TransportError> {
    let saved = crate::cloud_account::load_stored_account().ok().flatten();
    held_unless_signed_in_to(
        saved.as_ref().map(|saved| saved.server_url.as_str()),
        server_url,
    )
}

fn held_unless_signed_in_to(saved: Option<&str>, server_url: &str) -> Result<(), TransportError> {
    let reason = match saved {
        Some(saved) if saved.trim_end_matches('/') == server_url.trim_end_matches('/') => {
            return Ok(());
        }
        Some(_) => "The saved Buwiz sign-in is for another server. Sign in again to send returns.",
        None => "Sign in to send returns.",
    };
    Err(TransportError::RelayHeld {
        reason: reason.into(),
        wait_until: None,
    })
}

/// A relay send that is ready to go: the trust list verified, this device
/// registered, the sealing key checked. Nothing about the return has left
/// this computer yet.
pub(crate) struct RelaySession {
    submitter: Arc<Submitter>,
}

/// Make the relay ready, before the queue claims the return. Everything here
/// talks to Buwiz only, so a failure is never a failed try of the return: it
/// is held ([`TransportError::RelayHeld`]) and goes again later.
pub(crate) async fn open() -> Result<RelaySession, TransportError> {
    let submitter = submitter()?;
    signed_in_to(&submitter.config().server_url)?;
    submitter
        .ready()
        .await
        .map_err(|error| TransportError::RelayHeld {
            wait_until: error.wait_until(),
            reason: error.to_string(),
        })?;
    Ok(RelaySession { submitter })
}

impl RelaySession {
    /// Send the plain return `xml` under its eBIRForms file name. `note`
    /// keeps the client's pending send on the claimed return before anything
    /// could reach the BIR ([`bir_submit::Submitter::send_noting`]).
    pub(crate) async fn store(
        self,
        filename: &str,
        xml: &[u8],
        note: bir_submit::Note<'_>,
    ) -> Result<StoreReport, TransportError> {
        let name = crate::transport::iaf_basename(filename);
        // This device carries the upload: a phone stays awake and asks for time until the send ends, however it ends.
        let _in_flight = crate::send_in_flight::begin();
        match tokio::time::timeout(SEND_LIMIT, self.submitter.send_noting(name, xml, note)).await {
            Ok(sent) => send_outcome(sent),
            Err(_) => Err(TransportError::RelayUnknown(
                "Sending took longer than 10 minutes and was stopped. The BIR may have received this return."
                    .into(),
            )),
        }
    }
}

/// What the Buwiz server says of a send this app noted and did not see end
/// ([`settle`]).
#[derive(Debug)]
pub(crate) enum Settlement {
    /// A receipt that verified: finish the return as a send that went.
    Sent(StoreReport),
    /// No receipt: the outcome the send itself would have had.
    Failed(TransportError),
    /// Nothing to go on yet (the server cannot be asked, the person is signed
    /// out or signed in elsewhere, the submission is still under way).
    NotYet,
    /// The server does not know the submission, or the record is unreadable.
    Gone,
}

/// Ask the Buwiz server how the noted send `pending` (the stored
/// `bir_submit::PendingSend` JSON) ended. Only through the client this
/// build may send through ([`submitter`]: a development build's rules hold),
/// and only with a sign-in for that server: otherwise nothing is asked.
pub(crate) async fn settle(pending: &str) -> Settlement {
    let Ok(pending) = serde_json::from_str::<bir_submit::PendingSend>(pending) else {
        return Settlement::Gone;
    };
    let Ok(submitter) = submitter() else {
        return Settlement::NotYet;
    };
    if signed_in_to(&submitter.config().server_url).is_err() {
        return Settlement::NotYet;
    }
    settlement(submitter.settle(&pending).await)
}

/// The client's answer as the queue's, through [`send_outcome`]. Pure.
pub(crate) fn settlement(settled: bir_submit::Settled) -> Settlement {
    match settled {
        bir_submit::Settled::Sent(sent) => match send_outcome(Ok(*sent)) {
            Ok(report) => Settlement::Sent(report),
            Err(error) => Settlement::Failed(error),
        },
        bir_submit::Settled::Failed(error) => Settlement::Failed(send_failure(error)),
        bir_submit::Settled::NotYet => Settlement::NotYet,
        bir_submit::Settled::Gone => Settlement::Gone,
    }
}

/// The client's answer as the queue's outcome. Pure, so every case is
/// tested.
///
/// - A receipt: sent. `simulated` when the server did not send to the BIR
///   (its `dry_run` or `lab` mode).
/// - Not sent with a reason no retry can change (a TIN refusal, a file that
///   does not match its name): refused, back to Draft with that reason.
/// - Not sent otherwise, or a counted failure of the BIR's way in: a failed
///   try, retried later.
/// - Not a try: held.
/// - Maybe sent: unknown. Never anything that lets the return go again.
pub(crate) fn send_outcome(sent: Result<Sent, SendError>) -> Result<StoreReport, TransportError> {
    match sent {
        Ok(sent) => Ok(StoreReport {
            simulated: sent.receipt.is_test() || sent.receipt.mode != "live",
            relay_receipt: Some(sent.receipt_jws),
        }),
        Err(error) => Err(send_failure(error)),
    }
}

/// The relay's `result` for a test TIN, shared by the three links of "a relay dry_run result never reads as filed"
/// (`bir-submit/tests/fixtures/relay-result-dry-run.json`): status `dry_run`, and the claims of its dry-run receipt for
/// `<xml/>` on ticket `jti-1`. bir-submit's `a_relay_dry_run_result_with_its_signed_receipt_is_a_verified_test_send`
/// signs those claims with its test key and proves the client's send verifies them and hands them over unchanged.
#[cfg(any(test, feature = "test-support"))]
pub const DRY_RUN_RESULT_FIXTURE: &str =
    include_str!("../../bir-submit/tests/fixtures/relay-result-dry-run.json");

/// What the queue keeps of [`DRY_RUN_RESULT_FIXTURE`]: the client's [`Sent`] of it through [`send_outcome`], exactly
/// as a send through the Buwiz relay hands it to the worker. Its receipt is the fixture's claims in compact form; the
/// queue keeps it as it comes and checks no signature (the client did, link 1).
#[cfg(any(test, feature = "test-support"))]
pub fn dry_run_result_fixture_report() -> StoreReport {
    let result: serde_json::Value =
        serde_json::from_str(DRY_RUN_RESULT_FIXTURE).expect("the fixture is JSON");
    let claims = &result["receipt_claims"];
    let part = |value: &serde_json::Value| {
        data_encoding::BASE64URL_NOPAD.encode(value.to_string().as_bytes())
    };
    let header = serde_json::json!({"alg": "EdDSA", "typ": bir_submit::receipt::TYP});
    let sent = Sent {
        submission_id: "s1".into(),
        receipt: serde_json::from_value(claims.clone()).expect("receipt claims"),
        receipt_jws: format!("{}.{}.verified-by-the-client", part(&header), part(claims)),
    };
    send_outcome(Ok(sent)).expect("a receipt is a send that went")
}

/// [`send_outcome`] of a send that ended without a receipt.
fn send_failure(error: SendError) -> TransportError {
    match error {
        SendError::NotSent(reason) if is_final_reason(&reason) => {
            TransportError::Refused(TinRefusal::capped(&reason, REFUSAL_CHARS))
        }
        SendError::NotSent(reason) | SendError::Counted { reason, .. } => {
            TransportError::RelayFailed(reason)
        }
        SendError::NotATry { reason, wait_until } => {
            TransportError::RelayHeld { reason, wait_until }
        }
        SendError::MaybeSent(reason) => TransportError::RelayUnknown(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::queueable::NoReceipt;
    use bir_submit::Blame;

    /// A fixed access token (the mock's), as the host's sign-in would give it.
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
            Some("filer@example.test".into())
        }
        fn session_ended(&self) {}
    }

    #[test]
    fn a_practice_account_registers_no_device_and_no_client_is_not_a_failure() {
        let asked = std::cell::Cell::new(false);
        let outcome = register_with(true, || {
            asked.set(true);
            Err(TransportError::Config("unused".into()))
        });
        assert_eq!(outcome, Registration::Practice);
        assert!(!asked.get(), "no client is made for a practice account");
        assert_eq!(
            register_with(false, || Err(TransportError::Config(
                "no test trust key".into()
            ))),
            Registration::NoClient(TransportError::Config("no test trust key".into()).to_string())
        );
        // Through the real entry point in a test run: a debug build has no test trust key here, so no client; nothing
        // is sent and nothing fails.
        if !crate::transport::practice_account() {
            assert!(matches!(
                register_after_sign_in(),
                Registration::NoClient(_) | Registration::Failed(_)
            ));
        }
    }

    /// Against the in-repo mock: after an email sign-in the device is added once with the install id; a second time
    /// is already done (the kept id, or the server's 409); a dead server is a logged failure.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn after_a_sign_in_the_device_is_added_once_and_409_is_done() {
        use crate::email_sign_in::{Intent, new_secret, start, verify};
        use crate::mock_account::{MockAccount, MockOptions};
        let server =
            MockAccount::start_with("127.0.0.1:0".parse().unwrap(), MockOptions::default())
                .await
                .unwrap();
        let url = server.url();
        let install = "install-id-of-this-test-0001";
        let outcome = std::thread::spawn(move || {
            let mut http = crate::cloud_account::BlockingHttp::new();
            let secret = new_secret();
            let started = start(
                &mut http,
                &url,
                "filer@example.test",
                Intent::SignIn,
                &secret,
                "macos",
                install,
                None,
            )
            .unwrap();
            let tokens = verify(
                &mut http,
                &url,
                &started.request_id,
                &secret,
                crate::mock_account::DEFAULT_CODE,
            )
            .unwrap();
            let client = |store: Arc<bir_submit::secrets::MemoryStore>| {
                let mut config = SubmitConfig::new(&url).unwrap();
                config.install_id = Some(install.into());
                config.platform = Some("macos".into());
                use bir_submit::secrets::SecretStore;
                struct Shared(Arc<bir_submit::secrets::MemoryStore>);
                impl bir_submit::secrets::SecretStore for Shared {
                    fn get(
                        &self,
                        name: &str,
                    ) -> Result<Option<String>, bir_submit::secrets::StoreError>
                    {
                        self.0.get(name)
                    }
                    fn set(
                        &self,
                        name: &str,
                        value: &str,
                    ) -> Result<(), bir_submit::secrets::StoreError> {
                        self.0.set(name, value)
                    }
                    fn delete(&self, name: &str) -> Result<(), bir_submit::secrets::StoreError> {
                        self.0.delete(name)
                    }
                }
                Arc::new(Submitter::new(
                    config,
                    Box::new(Shared(store)),
                    Arc::new(Fixed(tokens.access_token.clone())),
                ))
            };
            let store = Arc::new(bir_submit::secrets::MemoryStore::default());
            let first = register_with(false, || Ok(client(store.clone())));
            let again = register_with(false, || Ok(client(store.clone())));
            // A fresh store (the id forgotten) on the same sign-in: the server's 409 is "already done".
            let fresh = register_with(false, || {
                Ok(client(
                    Arc::new(bir_submit::secrets::MemoryStore::default()),
                ))
            });
            (first, again, fresh)
        })
        .join()
        .unwrap();
        assert_eq!(
            outcome,
            (
                Registration::Registered,
                Registration::AlreadyDone,
                Registration::AlreadyDone
            )
        );
        assert_eq!(server.registered_devices(), 1);
        let (_, platform, _) = server.registered(install).unwrap();
        assert_eq!(platform, "macos");
        drop(server);
        let dead = std::thread::spawn(|| {
            register_with(false, || {
                let config = SubmitConfig::new("http://127.0.0.1:9").unwrap();
                Ok(Arc::new(Submitter::new(
                    config,
                    Box::new(bir_submit::secrets::MemoryStore::default()),
                    Arc::new(Fixed("token".into())),
                )))
            })
        })
        .join()
        .unwrap();
        assert!(matches!(dead, Registration::Failed(_)), "{dead:?}");
    }

    #[test]
    fn a_development_build_sends_only_through_a_server_on_this_computer() {
        for url in [
            "http://localhost:3008",
            "http://127.0.0.1:3008",
            "http://[::1]:3008",
            "https://localhost",
        ] {
            assert!(server_allowed(url, true).is_ok(), "{url}");
        }
        for url in [
            "https://api.buwiz.com",
            "https://localhost.example",
            "http://192.0.2.1:3008",
            "not a url",
        ] {
            assert!(
                matches!(server_allowed(url, true), Err(TransportError::Config(_))),
                "{url}"
            );
        }
        assert!(server_allowed("https://api.buwiz.com", false).is_ok());
    }

    #[test]
    fn a_token_is_only_offered_to_the_server_the_sign_in_is_for() {
        assert!(
            held_unless_signed_in_to(Some("http://localhost:3008"), "http://localhost:3008/")
                .is_ok()
        );
        for saved in [None, Some("https://api.buwiz.com")] {
            let held = held_unless_signed_in_to(saved, "http://localhost:3008").unwrap_err();
            assert_eq!(
                held.before_claim(),
                NoReceipt::Held { until: None },
                "{saved:?}"
            );
        }
    }

    #[test]
    fn a_development_build_never_trusts_the_production_trust_key() {
        let production: Vec<String> = bir_submit::trust::TRUST_KEYS
            .iter()
            .map(|key| (*key).to_owned())
            .collect();
        let test_key = vec!["dGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXk".to_owned()];
        let mixed = [production.clone(), test_key.clone()].concat();
        assert!(trust_keys_allowed(&test_key, true).is_ok());
        for refused in [&production, &mixed, &Vec::new()] {
            assert!(
                matches!(
                    trust_keys_allowed(refused, true),
                    Err(TransportError::Config(_))
                ),
                "{refused:?}"
            );
        }
        assert!(trust_keys_allowed(&production, false).is_ok());
    }

    /// The server and the trust keys are read through `config::var` (an Android development build's baked list),
    /// and that path keeps both rules: on this computer, and a test key, never production's. Runs in a child
    /// process, so its overrides never reach another test.
    #[test]
    fn configured_server_and_trust_keys_keep_the_development_rules() {
        const CHILD: &str = "BIR_RELAY_CONFIG_CHILD";
        const TEST: &str =
            "relay::tests::configured_server_and_trust_keys_keep_the_development_rules";
        let test_key = "dGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXk";
        let production = bir_submit::trust::TRUST_KEYS[0];
        if std::env::var_os(CHILD).is_none() {
            // The environment names production's server and key: the overrides below must win over it.
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, "1")
                .env("BUWIZ_SERVER_URL", "https://api.buwiz.com")
                .env("BUWIZ_TRUST_KEYS", production)
                .env_remove("BUWIZ_ACCOUNT_URL")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        use crate::config::force;
        force("BUWIZ_SERVER_URL", Some("http://localhost:3008"));
        force("BUWIZ_TRUST_KEYS", Some(test_key));
        assert_eq!(
            crate::cloud_account::configured_server().unwrap().url,
            "http://localhost:3008"
        );
        let expected = if cfg!(debug_assertions) {
            vec![test_key.to_owned()]
        } else {
            vec![production.to_owned()]
        };
        assert_eq!(trust_keys(), expected);
        if !cfg!(debug_assertions) {
            return;
        }
        assert_eq!(server().unwrap(), "http://localhost:3008");
        for keys in [Some(production), None] {
            force("BUWIZ_TRUST_KEYS", keys);
            let refused = server().unwrap_err().to_string();
            assert!(refused.contains("test key"), "{keys:?}: {refused}");
        }
        force("BUWIZ_TRUST_KEYS", Some(test_key));
        for url in ["https://api.buwiz.com", "https://192.0.2.1:3008"] {
            force("BUWIZ_SERVER_URL", Some(url));
            let refused = server().unwrap_err().to_string();
            assert!(refused.contains("on this computer"), "{url}: {refused}");
        }
        // Plain http to another host is refused before that, as an address: never a sign-in in the clear.
        force("BUWIZ_SERVER_URL", Some("http://192.0.2.1:3008"));
        let refused = server().unwrap_err().to_string();
        assert!(refused.contains("https://"), "{refused}");
    }

    fn receipt(mode: &str, status: &str) -> Sent {
        Sent {
            submission_id: "s1".into(),
            receipt_jws: "header.payload.signature".into(),
            receipt: bir_submit::receipt::Receipt {
                sub: "u1".into(),
                tin: "123456789000".into(),
                form_type: "1601Cv2018".into(),
                period: "052024".into(),
                filename: "f.xml".into(),
                xml_sha256: "00".into(),
                payload_sha256: "00".into(),
                remote_path: "/1601Cv2018/f.xml".into(),
                mode: mode.into(),
                status: status.into(),
                iat: 0,
                jti: None,
            },
        }
    }

    #[test]
    fn only_a_live_receipt_is_a_real_send() {
        let live = send_outcome(Ok(receipt("live", "sent"))).unwrap();
        assert!(!live.simulated);
        assert_eq!(
            live.relay_receipt.as_deref(),
            Some("header.payload.signature")
        );
        for (mode, status) in [
            ("dry_run", "dry_run"),
            ("lab", "sent"),
            ("dry_run", "sent"),
            ("", "sent"),
        ] {
            let report = send_outcome(Ok(receipt(mode, status))).unwrap();
            assert!(report.simulated, "{mode} {status}");
            assert!(report.relay_receipt.is_some());
        }
    }

    #[test]
    fn a_refusal_goes_back_to_draft_with_its_code() {
        for code in [
            "tin_not_active",
            "tin_unverified",
            "tin_pending",
            "tin_needs_fix",
            "tin_paused",
            "tin_test_only",
            "account_banned",
            "account_practice",
            "return_mismatch",
        ] {
            let reason = format!("The server's sentence. ({code})");
            let error = send_outcome(Err(SendError::NotSent(reason.clone()))).unwrap_err();
            assert!(
                matches!(&error, TransportError::Refused(kept) if kept == &reason),
                "{code}: {error:?}"
            );
            assert_eq!(error.no_receipt(), NoReceipt::Refused, "{code}");
        }
    }

    #[test]
    fn a_long_refusal_is_cut_and_keeps_its_code() {
        let reason = format!("{} (tin_needs_fix)", "x".repeat(1000));
        let TransportError::Refused(kept) =
            send_outcome(Err(SendError::NotSent(reason))).unwrap_err()
        else {
            panic!("a refusal");
        };
        assert!(kept.ends_with(" (tin_needs_fix)"), "{kept}");
        assert!(kept.chars().count() < 330, "{}", kept.chars().count());
    }

    #[test]
    fn a_failure_of_the_birs_way_in_is_a_failed_try_never_draft() {
        for failed in [
            SendError::NotSent("the server did not send the return".into()),
            SendError::Counted {
                reason: "BIR's upload server did not answer".into(),
                blame: Blame::Upload,
            },
        ] {
            let error = send_outcome(Err(failed)).unwrap_err();
            assert_eq!(error.no_receipt(), NoReceipt::RelayFailed, "{error:?}");
        }
    }

    #[test]
    fn a_hold_is_not_a_try_and_keeps_the_servers_time() {
        let until = chrono::Utc::now() + chrono::Duration::minutes(40);
        let error = send_outcome(Err(SendError::NotATry {
            reason: "BIR is not accepting returns right now. (bir_offline)".into(),
            wait_until: Some(until),
        }))
        .unwrap_err();
        assert_eq!(error.no_receipt(), NoReceipt::Held { until: Some(until) });
        assert_eq!(error.before_claim(), NoReceipt::Held { until: Some(until) });
        assert_eq!(
            error.to_string(),
            "BIR is not accepting returns right now. (bir_offline)"
        );
    }

    #[test]
    fn maybe_sent_is_unknown_and_never_lets_the_return_go_again() {
        // Even when its text ends in a refusal code: the BIR may have it.
        for reason in [
            "The relay ended without a result.",
            "already submitted (tin_not_active)",
        ] {
            let error = send_outcome(Err(SendError::MaybeSent(reason.into()))).unwrap_err();
            assert!(
                matches!(error, TransportError::RelayUnknown(_)),
                "{error:?}"
            );
            assert_eq!(error.no_receipt(), NoReceipt::Unknown);
            assert!(!error.relay_did_not_accept());
        }
    }
}
