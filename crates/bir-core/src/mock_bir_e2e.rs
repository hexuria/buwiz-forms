//! The real queue worker (`background_cron::run_queue_tick`) and the real
//! transport (`BIR_SFTP_*` lab host) against the mock BIR server, without
//! the GUI. Demo taxpayer and fake data only; every address is loopback.
//!
//! The state each failure leaves is the worker's rule, cited per test:
//! - a session that fails before the claim (connect / login) stays
//!   unclaimed and backs off ("Claim only after the SFTP session is open,
//!   immediately before PUT. Connect/login timeouts stay unclaimed and use
//!   the existing retry CAS." in `process_queued_1601c_with_transport`);
//! - any PUT error after the claim keeps the claim: "outcome is unknown and
//!   support-assisted manual reconciliation is required", never retried.

use crate::background_cron::{get_active_jobs, run_queue_tick};
use crate::db::{Database, ReceiptConfirmationOutcome};
use crate::forms::FilingStatus;
use crate::forms::form_1601c::Form1601CDraft;
use crate::mock_bir::{Behaviour, DelayAt, Failure, MockConfig, MockServer};
use crate::profile::TaxpayerProfile;
use crate::transport::{ResolvedSftpTarget, SftpEndpointSource, resolve_sftp_endpoint};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const FORM_TYPE: &str = "1601Cv2018";
const YEAR: u16 = 2024;

/// Juana Santos, TIN 000-123-453-00000, the demo taxpayer. Email tracking
/// stays off, so no inbox poll is scheduled and nothing leaves loopback.
fn juana() -> TaxpayerProfile {
    serde_json::from_value(serde_json::json!({
        "id": null,
        "full_name": "Juana Santos",
        "tin": {"segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000"},
        "rdo_code": "040",
        "line_of_business": "Retail",
        "registered_address": "1 Example Street, Pasig",
        "zip_code": "1600",
        "phone": "0280000000",
        "email": "juana@example.com",
        "default_form_type": "2551Qv2018",
        "taxpayer_type": "Individual",
        "business_start_date": "2010-01-01"
    }))
    .unwrap()
}

struct Harness {
    mock: MockServer,
    _root: tempfile::TempDir,
    db: Arc<Mutex<Database>>,
    queued: Form1601CDraft,
}

impl Harness {
    /// A mock with `behaviour` and a database holding Juana's queued 1601C
    /// for `month` (each test uses its own month, so worker job keys differ).
    async fn new(behaviour: Behaviour, month: u8) -> Self {
        let root = tempfile::tempdir().expect("temp dir");
        let mut config = MockConfig::new(root.path());
        config.bind = SocketAddr::from(([127, 0, 0, 1], 0));
        config.behaviour = behaviour;
        let mock = MockServer::start(config).await.expect("mock BIR server");
        assert!(mock.addr().ip().is_loopback());

        let profile = juana();
        assert!(!profile.is_email_tracking_active());
        let db = Database::open_in_memory_for_tests().expect("test database");
        db.save_profile(profile.clone()).expect("demo profile");
        let mut queued = Form1601CDraft::new_from_profile(&profile, YEAR, month);
        queued.any_taxes_withheld = false;
        queued
            .transition_to_queued()
            .expect("the demo 1601C should queue");
        db.save_queued_1601c_draft(&queued)
            .expect("queued 1601C should persist");
        Self {
            mock,
            _root: root,
            db: Arc::new(Mutex::new(db)),
            queued,
        }
    }

    fn remote_path(&self) -> String {
        format!("/{FORM_TYPE}/{}", self.queued.default_submission_filename())
    }

    fn stored(&self) -> Form1601CDraft {
        self.db
            .lock()
            .unwrap()
            .get_1601c_draft(
                &self.queued.tin,
                self.queued.taxable_year,
                self.queued.month,
            )
            .unwrap()
            .expect("the 1601C row stays persisted")
    }

    async fn run_worker(&self) {
        let job_key = format!(
            "form:1601C:{}:{}:{}",
            juana().tin.full(),
            self.queued.taxable_year,
            self.queued.month
        );
        run_worker(&self.mock, &self.db, FORM_TYPE, &self.queued.tin, &job_key).await;
    }

    /// The mock's confirmation message through the steps the IMAP fetcher
    /// runs on a fetched message (`email::fetcher::fetch_with_auth`):
    /// parse, text body, BIR receipt parser, save, confirm.
    fn deliver_receipt(&self) -> ReceiptConfirmationOutcome {
        let path = self
            .mock
            .receipt_path(&self.queued.default_submission_filename());
        let raw = std::fs::read(&path).expect("the mock wrote a confirmation");
        let message = mail_parser::MessageParser::default()
            .parse(&raw)
            .expect("a valid message");
        let text = message
            .body_text(0)
            .map(|text| text.into_owned())
            .unwrap_or_default();
        let email_received_at = message.date().map(|date| date.to_rfc3339());
        let receipt = crate::receipt::parse_bir_receipt_email(&text, None)
            .expect("the real receipt parser accepts the mock confirmation");
        let db = self.db.lock().unwrap();
        let (saved, _) = db
            .save_submission_receipt_with_email_date(&receipt, email_received_at.as_deref())
            .unwrap();
        db.confirm_submission_from_receipt(&saved).unwrap()
    }

    fn assert_nothing_stored(&self) {
        assert!(self.mock.received(&self.remote_path()).is_none());
        let path = self.mock.upload_path(&self.remote_path()).unwrap();
        assert!(!path.exists(), "{} must not exist", path.display());
    }

    fn assert_claimed_unknown_outcome(&self) {
        let claimed = self.stored();
        assert_eq!(claimed.status, FilingStatus::Queued);
        assert!(claimed.submission_claim_token.is_some());
        assert!(claimed.submission_claimed_at.is_some());
        assert_eq!(claimed.submission_attempts, 0);
        assert!(
            claimed
                .submission_error
                .as_deref()
                .is_some_and(|message| message.contains("outcome pending")),
            "{:?}",
            claimed.submission_error
        );
    }
}

/// One worker tick with the app's lab-host environment pointing at the
/// mock, waiting for the spawned submission job (`job_key`) to finish.
async fn run_worker(
    mock: &MockServer,
    db: &Arc<Mutex<Database>>,
    form_type: &str,
    tin: &str,
    job_key: &str,
) {
    let port = mock.addr().port().to_string();
    let pin = mock.host_key_sha256().to_string();
    let vars: Vec<(&str, Option<String>)> = vec![
        ("EBIR_TEST_ENV", None),
        ("BIR_SFTP_DRY_RUN", None),
        ("BIR_SFTP_LIVE", None),
        ("BIR_SFTP_USER", None),
        ("BIR_SFTP_FOLDER", None),
        ("BIR_SFTP_ACCEPT_ANY_HOST_KEY", None),
        ("BIR_SFTP_HOST", Some("127.0.0.1".into())),
        ("BIR_SFTP_PORT", Some(port)),
        (
            "BIR_SFTP_USERNAME",
            Some(crate::mock_bir::DEFAULT_USERNAME.into()),
        ),
        (
            "BIR_SFTP_PASSWORD",
            Some(crate::mock_bir::DEFAULT_PASSWORD.into()),
        ),
        ("BIR_SFTP_HOST_KEY_SHA256", Some(pin)),
    ];
    temp_env::async_with_vars(vars, async {
        // The transport resolves the lab host, on loopback, never the
        // dispatcher.
        match resolve_sftp_endpoint(form_type, tin)
            .await
            .expect("lab host resolves")
        {
            ResolvedSftpTarget::Live { endpoint, source } => {
                assert_eq!(source, SftpEndpointSource::Env);
                let ip: std::net::IpAddr = endpoint.host.parse().unwrap();
                assert!(ip.is_loopback(), "{ip}");
            }
            ResolvedSftpTarget::DryRun { .. } => panic!("expected the mock, got a dry run"),
        }
        run_queue_tick(db.clone()).await;
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while get_active_jobs().lock().unwrap().contains(job_key) {
            assert!(
                std::time::Instant::now() < deadline,
                "worker job did not finish; mock events {:?}",
                mock.events()
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
}

fn events_contain(mock: &MockServer, needle: &str) -> bool {
    mock.events().iter().any(|line| line.contains(needle))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_return_ends_submitted_with_the_exact_file_in_the_mock() {
    let h = Harness::new(Behaviour::default(), 1).await;
    h.run_worker().await;

    let submitted = h.stored();
    assert_eq!(
        submitted.status,
        FilingStatus::Submitted,
        "{:?}",
        h.mock.events()
    );
    assert!(submitted.submission_claim_token.is_none());
    assert_eq!(
        submitted.submission_filename.as_deref(),
        Some(h.queued.default_submission_filename().as_str())
    );

    // What arrived over SFTP is what is on disk, and it decrypts to the XML
    // the worker recorded as sent.
    let wire = h
        .mock
        .received(&h.remote_path())
        .expect("mock got the file");
    let disk = std::fs::read(h.mock.upload_path(&h.remote_path()).unwrap()).unwrap();
    assert_eq!(disk, wire);
    let xml = crate::crypto::decrypt_and_decompress(&wire, crate::crypto::BIR_IAF_PASSPHRASE)
        .expect("the upload is a BIR IAF");
    assert_eq!(
        Some(String::from_utf8(xml).unwrap()),
        submitted.submitted_payload
    );
    assert_eq!(
        Some(h.queued.try_to_bir_xml_payload().unwrap()),
        submitted.submitted_payload
    );
    assert!(events_contain(&h.mock, "login ok user=mock-filer"));
    assert!(events_contain(&h.mock, "file received"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mock_confirmation_moves_submitted_to_confirmed_through_the_real_receipt_rules() {
    let h = Harness::new(Behaviour::default(), 2).await;
    h.run_worker().await;
    assert_eq!(h.stored().status, FilingStatus::Submitted);

    assert_eq!(h.deliver_receipt(), ReceiptConfirmationOutcome::Confirmed);
    let confirmed = h.stored();
    assert_eq!(confirmed.status, FilingStatus::Confirmed);
    // A second delivery of the same message changes nothing.
    assert_eq!(h.deliver_receipt(), ReceiptConfirmationOutcome::Ignored);
}

/// Login refused is before the claim: unclaimed, one attempt, backed off.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refused_login_stays_unclaimed_and_backs_off_before_the_claim() {
    let h = Harness::new(
        Behaviour {
            failure: Failure::RefuseLogin,
            ..Behaviour::default()
        },
        3,
    )
    .await;
    h.run_worker().await;

    let failed = h.stored();
    assert_eq!(failed.status, FilingStatus::Queued);
    assert!(failed.submission_claim_token.is_none());
    assert!(failed.submission_claimed_at.is_none());
    assert_eq!(failed.submission_attempts, 1);
    assert!(failed.next_retry_at.is_some());
    assert!(
        failed
            .submission_error
            .as_deref()
            .is_some_and(|message| message.contains("no PUT attempted")),
        "{:?}",
        failed.submission_error
    );
    h.assert_nothing_stored();
    assert!(events_contain(
        &h.mock,
        "login refused (injected refuse-login)"
    ));
}

/// The upload is refused before any byte is stored, but after the claim:
/// the worker cannot tell, so the outcome is unknown and the claim stays.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upload_refused_before_storing_keeps_the_claim_as_an_unknown_outcome() {
    let h = Harness::new(
        Behaviour {
            failure: Failure::FailBeforeStore,
            ..Behaviour::default()
        },
        4,
    )
    .await;
    h.run_worker().await;

    h.assert_claimed_unknown_outcome();
    h.assert_nothing_stored();

    // Never retried automatically: a second tick sends nothing.
    h.run_worker().await;
    h.assert_claimed_unknown_outcome();
    assert_eq!(
        h.mock
            .events()
            .iter()
            .filter(|line| line.contains("connection from"))
            .count(),
        1
    );
}

/// Stored, then the close fails: the ambiguous case. The claim stays, and
/// the confirmation BIR would send is ignored, because a receipt only
/// confirms a Submitted return (`Database::confirm_1601c_from_receipt`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failure_after_storing_keeps_the_claim_and_its_receipt_is_ignored() {
    let h = Harness::new(
        Behaviour {
            failure: Failure::FailAfterStore,
            ..Behaviour::default()
        },
        5,
    )
    .await;
    h.run_worker().await;

    h.assert_claimed_unknown_outcome();
    let stored = h
        .mock
        .received(&h.remote_path())
        .expect("the file IS stored");
    assert!(
        crate::crypto::decrypt_and_decompress(&stored, crate::crypto::BIR_IAF_PASSPHRASE).is_ok()
    );

    assert_eq!(h.deliver_receipt(), ReceiptConfirmationOutcome::Ignored);
    h.assert_claimed_unknown_outcome();
}

/// A slow login is before the claim and has no client timeout: it ends
/// Submitted once the mock answers.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_login_still_ends_submitted() {
    let h = Harness::new(
        Behaviour {
            delay: Duration::from_secs(2),
            delay_at: DelayAt::Login,
            ..Behaviour::default()
        },
        6,
    )
    .await;
    let started = std::time::Instant::now();
    h.run_worker().await;
    assert!(started.elapsed() >= Duration::from_secs(2));
    assert_eq!(h.stored().status, FilingStatus::Submitted);
    assert!(events_contain(
        &h.mock,
        "slow: waiting 2s before answering login"
    ));
}

/// The SFTP client gives each request 10 seconds (russh-sftp's default).
/// A server that stores the file but answers the close after that is the
/// ambiguous case again: claimed, outcome unknown, file present.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_answered_after_the_client_timeout_is_an_unknown_outcome_with_the_file_stored() {
    let h = Harness::new(
        Behaviour {
            delay: Duration::from_secs(11),
            delay_at: DelayAt::Close,
            ..Behaviour::default()
        },
        7,
    )
    .await;
    h.run_worker().await;

    h.assert_claimed_unknown_outcome();
    assert!(h.mock.received(&h.remote_path()).is_some());
}

/// The 2551Q branch of the worker is its own copy of the claim logic; the
/// happy path through it ends Submitted with the file in the mock.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_2551q_ends_submitted_with_the_file_in_the_mock() {
    use crate::forms::form_2551q::{Form2551QDraft, Item13Election};

    let root = tempfile::tempdir().expect("temp dir");
    let mut config = MockConfig::new(root.path());
    config.bind = SocketAddr::from(([127, 0, 0, 1], 0));
    let mock = MockServer::start(config).await.expect("mock BIR server");

    let mut profile = juana();
    profile.ensure_profile_version_ledger();
    let db = Database::open_in_memory_for_tests().expect("test database");
    db.save_profile(profile.clone()).expect("demo profile");
    let mut queued = Form2551QDraft::new_from_effective_profile(&profile, 2026, 1);
    queued.item_13_election = Item13Election::Graduated;
    queued
        .transition_to_queued()
        .expect("the demo 2551Q should queue");
    db.save_queued_2551q_draft_and_election(&queued)
        .expect("queued 2551Q should persist");
    let db = Arc::new(Mutex::new(db));

    let job_key = format!("form:2551Q:{}:2026:1", juana().tin.full());
    run_worker(&mock, &db, "2551Qv2018", &queued.tin, &job_key).await;

    let submitted = db
        .lock()
        .unwrap()
        .get_2551q_draft(&queued.tin, 2026, 1)
        .unwrap()
        .expect("the 2551Q row stays persisted");
    assert_eq!(
        submitted.status,
        FilingStatus::Submitted,
        "{:?}",
        mock.events()
    );
    let remote = format!("/2551Qv2018/{}", queued.default_submission_filename());
    let wire = mock.received(&remote).expect("mock got the 2551Q file");
    assert_eq!(
        std::fs::read(mock.upload_path(&remote).unwrap()).unwrap(),
        wire
    );
    assert!(
        crate::crypto::decrypt_and_decompress(&wire, crate::crypto::BIR_IAF_PASSPHRASE).is_ok()
    );
}
