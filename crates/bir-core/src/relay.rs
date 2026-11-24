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

/// The Buwiz server this build may send through, or why it may not: the configured server, which for a
/// development build must be on this computer and trusted through a test key. No request is made.
pub fn server() -> Result<String, TransportError> {
    let server = crate::cloud_account::configured_server()
        .map_err(|error| TransportError::Config(error.to_string()))?;
    server_allowed(&server.url, cfg!(debug_assertions))?;
    trust_keys_allowed(&bir_submit::trust::pinned_keys(), cfg!(debug_assertions))?;
    Ok(server.url)
}

/// Where the device key and id live when the keychain is not used: beside the database, as the saved sign-in
/// is. A scratch database (a development, test or agent run) gets a scratch device, never a file in the app's
/// real data folder.
fn secrets_dir() -> std::path::PathBuf {
    let database = crate::db::app_database_path();
    database
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
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
    let config = SubmitConfig::new(&server.url)
        .map_err(|error| TransportError::Config(error.to_string()))?;
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
    /// Send the plain return `xml` under its eBIRForms file name.
    pub(crate) async fn store(
        self,
        filename: &str,
        xml: &[u8],
    ) -> Result<StoreReport, TransportError> {
        let name = crate::transport::iaf_basename(filename);
        match tokio::time::timeout(SEND_LIMIT, self.submitter.send(name, xml)).await {
            Ok(sent) => send_outcome(sent),
            Err(_) => Err(TransportError::RelayUnknown(
                "Sending took longer than 10 minutes and was stopped. The BIR may have received this return."
                    .into(),
            )),
        }
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
        Err(SendError::NotSent(reason)) if is_final_reason(&reason) => Err(
            TransportError::Refused(TinRefusal::capped(&reason, REFUSAL_CHARS)),
        ),
        Err(SendError::NotSent(reason)) | Err(SendError::Counted { reason, .. }) => {
            Err(TransportError::RelayFailed(reason))
        }
        Err(SendError::NotATry { reason, wait_until }) => {
            Err(TransportError::RelayHeld { reason, wait_until })
        }
        Err(SendError::MaybeSent(reason)) => Err(TransportError::RelayUnknown(reason)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::queueable::NoReceipt;
    use bir_submit::Blame;

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
