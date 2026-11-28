use crate::db::{Claim1601CSubmissionResult, Claim2551QSubmissionResult, Database};
use crate::forms::form_1601c::Form1601CDraft;
use crate::forms::form_2551q::Form2551QDraft;
use crate::forms::queueable::{NoReceipt, QueueableForm, QueueableKind};
use crate::forms::{FilingStatus, FormDraftSummary};
use crate::job_display::{FilingJobLabel, known_job_email};
use crate::lock_timing;
use crate::profile::TaxpayerProfile;
use chrono::Utc;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// The wake channel of the worker loop that runs now; empty while none runs. Each loop puts its own in and takes it
/// out when it ends (or is dropped), so a later loop in the same process is woken, not the one before it.
static WAKE_TX: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);
static ACTIVE_JOBS: OnceLock<Arc<Mutex<HashSet<String>>>> = OnceLock::new();
/// Counts [`request_shutdown`] calls. A loop stops when it moves past the value it started with, so a request stops
/// the loops running then and never one started later.
static SHUTDOWN_GENERATION: AtomicU64 = AtomicU64::new(0);
/// The value of [`SHUTDOWN_GENERATION`] the most recent loop started with ([`shutdown_requested`]).
static LATEST_WORKER_GENERATION: AtomicU64 = AtomicU64::new(0);
/// The shell holds this until a Buwiz account is signed in. Default false so
/// bir-core and bir-shell cron tests keep filing. The shell sets it true
/// before the worker starts.
static SUBMISSION_PAUSED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WakeDispatchStatus {
    Delivered,
    AlreadyPending,
    WorkerNotRunning,
    WorkerClosed,
}

/// One worker loop's own stop signal ([`run_cron_jobs`]). Its owner keeps a clone and calls [`CronStop::stop`]; the loop
/// returns after the tick it is on, and at once while it waits for its next one. It only ever stops the loop it was
/// given to, so a worker started after it is not born stopped.
#[derive(Clone, Default)]
pub struct CronStop(Arc<CronStopInner>);

#[derive(Default)]
struct CronStopInner {
    stopped: AtomicBool,
    notify: tokio::sync::Notify,
}

impl CronStop {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask the loop to stop. A second call does nothing more.
    pub fn stop(&self) {
        self.0.stopped.store(true, Ordering::SeqCst);
        self.0.notify.notify_one();
    }

    pub fn is_stopped(&self) -> bool {
        self.0.stopped.load(Ordering::SeqCst)
    }

    async fn stopped(&self) {
        while !self.is_stopped() {
            self.0.notify.notified().await;
        }
    }
}

pub fn get_active_jobs() -> Arc<Mutex<HashSet<String>>> {
    ACTIVE_JOBS
        .get_or_init(|| Arc::new(Mutex::new(HashSet::new())))
        .clone()
}

/// The key the submission worker holds in [`get_active_jobs`] while it sends one queued return:
/// `form:{form_code}:{tin}:{year}:{month or quarter or 0}`, with `tin` as the worker's profile writes it
/// (the 14 digits of `form_drafts.tin`) and the month and quarter of the return's
/// [`crate::forms::FormDraftSummary`]. The worker and every reader of "is this return being sent now"
/// build it here, so the two cannot drift.
pub fn submission_job_key(
    form_code: &str,
    tin: &str,
    taxable_year: u16,
    month: Option<u8>,
    quarter: Option<u8>,
) -> String {
    format!(
        "form:{form_code}:{tin}:{taxable_year}:{}",
        month.unwrap_or(quarter.unwrap_or(0))
    )
}

/// Stop every [`start_cron_jobs`] / [`run_cron_jobs`] loop running now, after the tick it is already on.
///
/// Wakes a loop that is waiting out its 60 second gap. A second call is a
/// no-op aside from another wake. A loop started after this call is not
/// stopped by it. The app's quit and an engine's end do not call this: they
/// stop their own worker through its [`CronStop`], so a later engine's loop is
/// never stopped by an earlier engine's end.
pub fn request_shutdown() {
    SHUTDOWN_GENERATION.fetch_add(1, Ordering::SeqCst);
    wake();
}

/// Whether [`request_shutdown`] has been asked for since the most recent loop started.
pub fn shutdown_requested() -> bool {
    SHUTDOWN_GENERATION.load(Ordering::SeqCst) != LATEST_WORKER_GENERATION.load(Ordering::SeqCst)
}

/// Skip queue ticks while the desktop account gate is locked.
///
/// Unlike [`request_shutdown`], this can be cleared. A later sign-in in the
/// same process files again.
pub fn set_submission_paused(paused: bool) {
    SUBMISSION_PAUSED.store(paused, Ordering::SeqCst);
    if !paused {
        // Quiet when the worker has not started yet. Launch unpauses before
        // the thread exists; the first tick runs as soon as it does.
        let _ = wake_with_status();
    }
}

pub fn submission_paused() -> bool {
    SUBMISSION_PAUSED.load(Ordering::SeqCst)
}

pub fn wake() {
    match wake_with_status() {
        WakeDispatchStatus::Delivered | WakeDispatchStatus::AlreadyPending => {}
        WakeDispatchStatus::WorkerNotRunning => {
            warn!("Background cron wake was deferred because the worker is not running yet");
        }
        WakeDispatchStatus::WorkerClosed => {
            warn!("Background cron wake was deferred because the worker channel is closed");
        }
    }
}

pub(crate) fn wake_with_status() -> WakeDispatchStatus {
    let slot = WAKE_TX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(tx) = slot.as_ref() else {
        return WakeDispatchStatus::WorkerNotRunning;
    };

    match tx.try_send(()) {
        Ok(()) => WakeDispatchStatus::Delivered,
        Err(mpsc::error::TrySendError::Full(())) => WakeDispatchStatus::AlreadyPending,
        Err(mpsc::error::TrySendError::Closed(())) => WakeDispatchStatus::WorkerClosed,
    }
}

/// A loop's wake channel in [`WAKE_TX`] for as long as the loop lives: taken out when it returns or is dropped
/// mid-tick, unless a later loop has put its own in.
struct WakeInstalled(mpsc::Sender<()>);

impl WakeInstalled {
    fn new(tx: mpsc::Sender<()>) -> Self {
        *WAKE_TX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(tx.clone());
        Self(tx)
    }
}

impl Drop for WakeInstalled {
    fn drop(&mut self) {
        let mut slot = WAKE_TX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if slot.as_ref().is_some_and(|tx| tx.same_channel(&self.0)) {
            *slot = None;
        }
    }
}

/// The queue worker loop until [`request_shutdown`] (the `bir` command line, `bir serve`).
pub fn start_cron_jobs(db: Arc<Mutex<Database>>) -> impl Future<Output = ()> + Send {
    run_cron_jobs(db, CronStop::new())
}

/// The queue worker loop until `stop` is asked (or [`request_shutdown`]). The app's engine owns `stop`, so the next
/// engine in the same process starts a loop of its own that nothing has stopped.
///
/// The loop's place among [`request_shutdown`] calls is taken here, when it is called, not when it is first polled:
/// a request made after this call stops it even if the runtime has not run it yet.
pub fn run_cron_jobs(db: Arc<Mutex<Database>>, stop: CronStop) -> impl Future<Output = ()> + Send {
    let born = SHUTDOWN_GENERATION.load(Ordering::SeqCst);
    LATEST_WORKER_GENERATION.store(born, Ordering::SeqCst);
    run_cron_loop(db, stop, born)
}

async fn run_cron_loop(db: Arc<Mutex<Database>>, stop: CronStop, born: u64) {
    info!("Background cron engine started");
    requeue_jobs_left_running(&db);

    let stopping = || stop.is_stopped() || SHUTDOWN_GENERATION.load(Ordering::SeqCst) != born;
    let (tx, mut rx) = mpsc::channel(1);
    let _wake = WakeInstalled::new(tx);

    loop {
        if stopping() {
            break;
        }
        if submission_paused() {
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
                _ = rx.recv() => {}
                _ = stop.stopped() => {}
            }
            continue;
        }
        // We run a tick, then wait for either 60 seconds or a wake signal.
        run_queue_tick_until(db.clone(), &stop).await;
        if stopping() {
            break;
        }

        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
            _ = rx.recv() => {
                info!("Queue explicitly woken up by a wake signal.");
            }
            _ = stop.stopped() => {}
        }
    }
    info!("Background cron engine stopped");
}

/// Runs a single iteration of the background job queue.
/// Extracted from `start_cron_jobs` to allow deterministic unit testing
/// without being blocked by an infinite loop or thread sleep.
pub async fn run_queue_tick(db: Arc<Mutex<Database>>) {
    run_queue_tick_until(db, &CronStop::new()).await;
}

/// One tick whose blocking work (the calendar sync, an inbox poll) checks `stop` between its items and ends early.
async fn run_queue_tick_until(db: Arc<Mutex<Database>>, stop: &CronStop) {
    // Heartbeat log removed to prevent log spam. Job specific logs are emitted when jobs run.

    // Ensure we don't hold the DB lock across async network calls.
    // First, fetch profiles that have background cron enabled.
    let (profiles, global_cron_enabled) = {
        let db_guard = match lock_timing::lock(&db) {
            Ok(guard) => guard,
            Err(e) => {
                error!("Failed to lock database for cron: {}", e);
                return;
            }
        };
        let p = match db_guard.list_profiles() {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to fetch profiles in cron: {}", e);
                return;
            }
        };
        let c = db_guard
            .get_setting("background_cron_enabled")
            .unwrap_or(Some("true".to_string()))
            .map(|s| s == "true")
            .unwrap_or(true);
        (p, c)
    };

    let test_enabled = profiles.iter().any(|p| p.test_notification_enabled);
    if test_enabled {
        const TITLE: &str = "BIR Vault Daemon";
        const BODY: &str = "Hello! The background cron is active.";
        // The painted app delivers alert rows itself; headless posts directly.
        if let Ok(db_guard) = lock_timing::lock(&db) {
            let _ = db_guard.record_alert(
                None,
                crate::db::alert_kinds::CRON_HEARTBEAT,
                crate::db::AlertSeverity::Info,
                TITLE,
                BODY,
                crate::db::AlertAction::None,
            );
        }
        crate::notification::notify(crate::notification::NotificationKind::Heartbeat);
    }

    if global_cron_enabled {
        for profile in profiles {
            // Task A: Form Submission Retries
            process_submission_queue(&profile, db.clone()).await;
        }
    }

    // Task C: Generic Job Queue (Custom Cron & One-off commands)
    process_generic_jobs_until(db.clone(), stop).await;

    process_google_calendar_sync(db.clone(), stop).await;

    // Signal the desktop app that the database was modified.
    // On macOS: instant via NSDistributedNotificationCenter.
    // On Linux/Windows: no-op (desktop uses PRAGMA data_version polling).
    crate::ipc::post_db_changed();
}

async fn process_google_calendar_sync(db: Arc<Mutex<Database>>, stop: &CronStop) {
    const SIX_HOURS: i64 = 6 * 60 * 60;
    let should_sync = {
        let db = match lock_timing::lock(&db) {
            Ok(db) => db,
            Err(_) => return,
        };
        let connected = db
            .get_setting("google_calendar_connected_email")
            .ok()
            .flatten()
            .is_some_and(|email| !email.trim().is_empty());
        let has_links = db
            .list_profile_calendar_links()
            .is_ok_and(|links| !links.is_empty());
        if !connected {
            return;
        }
        if !has_links {
            let _ = db.set_setting("google_calendar_sync_requested", "false");
            return;
        }
        let requested = db
            .get_setting("google_calendar_sync_requested")
            .ok()
            .flatten()
            .as_deref()
            == Some("true");
        let last_sync = db
            .get_setting("google_calendar_last_sync_unix")
            .ok()
            .flatten()
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        requested || Utc::now().timestamp().saturating_sub(last_sync) >= SIX_HOURS
    };
    if !should_sync || !crate::google_calendar::google_calendar_configuration().configured {
        return;
    }

    // A `Weak` and the stop: a worker that is dropped while this blocking sync runs does not keep the database open
    // past the calendar it is on.
    let db_for_sync = Arc::downgrade(&db);
    let stop_for_sync = stop.clone();
    let run = match tokio::task::spawn_blocking(move || {
        crate::google_calendar::sync_all_profile_calendars_until(db_for_sync, &|| {
            stop_for_sync.is_stopped()
        })
    })
    .await
    {
        Ok(run) => run,
        Err(error) => {
            warn!("Google Calendar background sync task failed: {error}");
            return;
        }
    };
    if !run.complete {
        // Stopped part way (the worker is ending): not a sync. The last sync time and a pending "sync now" stay as
        // they were, so the next worker syncs every calendar again.
        info!(
            "Google Calendar sync stopped after {} of the linked calendars; not recorded",
            run.results.len()
        );
        return;
    }
    let results = run.results;
    if results.is_empty() {
        if let Ok(db) = lock_timing::lock(&db) {
            let _ = db.set_setting(
                "google_calendar_last_sync_unix",
                &Utc::now().timestamp().to_string(),
            );
            let _ = db.set_setting("google_calendar_sync_requested", "false");
        }
        return;
    }
    for (tin, result) in &results {
        match result {
            Ok(report) => info!(
                "Google Calendar sync for {}: {} inserted, {} updated, {} deleted",
                crate::logging::tin_for_log(tin),
                report.inserted,
                report.updated,
                report.deleted
            ),
            Err(error) => warn!(
                "Google Calendar sync failed for {}: {}",
                crate::logging::tin_for_log(tin),
                error
            ),
        }
    }
    let all_succeeded = results.iter().all(|(_, result)| result.is_ok());
    if let Ok(db) = lock_timing::lock(&db) {
        if !all_succeeded {
            let _ = db.set_setting("google_calendar_sync_requested", "true");
            return;
        }
        let _ = db.set_setting(
            "google_calendar_last_sync_unix",
            &Utc::now().timestamp().to_string(),
        );
        let _ = db.set_setting("google_calendar_sync_requested", "false");
    }
}

/// Identity of one user-reviewed queue generation. The form fingerprint binds
/// submission fields; retry timestamp and attempt count distinguish a later
/// cancel/requeue of otherwise identical content.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Queued2551QRevision {
    fingerprint: Option<String>,
    next_retry_at: Option<String>,
    submission_attempts: u32,
}

enum Queued2551QPreparation {
    Ready {
        draft: Form2551QDraft,
        revision: Queued2551QRevision,
    },
    Rejected {
        draft: Form2551QDraft,
        revision: Queued2551QRevision,
        errors: Vec<(String, String)>,
    },
    /// The row was canceled, replaced, or requeued after this job was spawned.
    Superseded,
}

fn queued_2551q_revision(draft: &Form2551QDraft) -> Option<Queued2551QRevision> {
    (matches!(draft.status, FilingStatus::Queued)
        && draft.submission_claim_token.is_none()
        && draft.submission_claimed_at.is_none())
    .then(|| Queued2551QRevision {
        fingerprint: draft.queued_submission_fingerprint.clone(),
        next_retry_at: draft.next_retry_at.clone(),
        submission_attempts: draft.submission_attempts,
    })
}

/// Pure submission-boundary preparation used both after a job is spawned and
/// immediately before network I/O. The caller loads the draft and profile under
/// one database lock, so this helper never validates against the tick's stale
/// bulk profile snapshot.
fn prepare_queued_2551q(
    mut draft: Form2551QDraft,
    profile: &TaxpayerProfile,
    expected_revision: Option<&Queued2551QRevision>,
) -> Queued2551QPreparation {
    let Some(revision) = queued_2551q_revision(&draft) else {
        return Queued2551QPreparation::Superseded;
    };
    if expected_revision.is_some_and(|expected| expected != &revision) {
        return Queued2551QPreparation::Superseded;
    }

    if let Err(error) = draft.reconcile_with_effective_profile(profile) {
        draft.revert_to_draft();
        draft.last_error = Some(format!(
            "Submission blocked because the effective taxpayer profile is unresolved: {error}"
        ));
        return Queued2551QPreparation::Rejected {
            draft,
            revision,
            errors: vec![("profile_resolution".to_string(), error)],
        };
    }
    match draft.revalidate_queued_before_submission() {
        Ok(()) => Queued2551QPreparation::Ready { draft, revision },
        Err(errors) => Queued2551QPreparation::Rejected {
            draft,
            revision,
            errors,
        },
    }
}

/// Identity of one user-reviewed 1601C queue generation. Retry metadata makes
/// cancellation followed by an identical requeue distinguishable.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Queued1601CRevision {
    fingerprint: Option<String>,
    next_retry_at: Option<String>,
    submission_attempts: u32,
}

enum Queued1601CPreparation {
    Ready {
        draft: Form1601CDraft,
        revision: Queued1601CRevision,
    },
    Rejected {
        draft: Form1601CDraft,
        revision: Queued1601CRevision,
        errors: Vec<(String, String)>,
    },
    Superseded,
}

fn queued_1601c_revision(draft: &Form1601CDraft) -> Option<Queued1601CRevision> {
    (matches!(draft.status, FilingStatus::Queued)
        && draft.submission_claim_token.is_none()
        && draft.submission_claimed_at.is_none())
    .then(|| Queued1601CRevision {
        fingerprint: draft.queued_submission_fingerprint.clone(),
        next_retry_at: draft.next_retry_at.clone(),
        submission_attempts: draft.submission_attempts,
    })
}

fn prepare_queued_1601c(
    mut draft: Form1601CDraft,
    expected_revision: Option<&Queued1601CRevision>,
) -> Queued1601CPreparation {
    let Some(revision) = queued_1601c_revision(&draft) else {
        return Queued1601CPreparation::Superseded;
    };
    if expected_revision.is_some_and(|expected| expected != &revision) {
        return Queued1601CPreparation::Superseded;
    }

    match draft.revalidate_queued_before_submission() {
        Ok(()) => Queued1601CPreparation::Ready { draft, revision },
        Err(errors) => Queued1601CPreparation::Rejected {
            draft,
            revision,
            errors,
        },
    }
}

fn validation_reason(errors: &[(String, String)]) -> String {
    errors
        .iter()
        .map(|(field, message)| format!("{field}: {message}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn queued_retry_is_due(
    next_retry_at: Option<&str>,
    now: chrono::DateTime<Utc>,
) -> Result<bool, chrono::ParseError> {
    let Some(next_retry_at) = next_retry_at else {
        return Ok(true);
    };
    let next_retry = chrono::DateTime::parse_from_rfc3339(next_retry_at)?;
    Ok(now >= next_retry.with_timezone(&Utc))
}

trait SubmissionTransport {
    type Session: Send;

    fn session_source(&self, session: &Self::Session) -> crate::transport::SftpEndpointSource;

    fn open_session<'a>(
        &'a self,
        form_type: &'a str,
        tin: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Self::Session, crate::transport::TransportError>>
                + Send
                + 'a,
        >,
    >;

    /// Send the return. `payload` is the encrypted IAF file, what the BIR's SFTP server (and the mock relay)
    /// takes; `xml` is the plain return it was made from, what the Buwiz relay takes and seals itself. A send
    /// through the Buwiz relay keeps its progress with `note` (on the claimed return) before anything could reach
    /// the BIR; no other send calls it.
    fn store_session<'a>(
        &'a self,
        session: Self::Session,
        filename: &'a str,
        payload: &'a [u8],
        xml: &'a [u8],
        note: bir_submit::Note<'a>,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        crate::transport::StoreReport,
                        crate::transport::TransportError,
                    >,
                > + Send
                + 'a,
        >,
    >;

    /// Ask how the noted Buwiz relay send `pending` (the stored `bir_submit::PendingSend` JSON) ended.
    fn settle_pending<'a>(
        &'a self,
        pending: &'a str,
    ) -> Pin<Box<dyn Future<Output = crate::relay::Settlement> + Send + 'a>>;
}

struct NetworkSubmissionTransport;

/// Where the worker's send goes: over SFTP (the mock BIR, the mock relay, a dry run) or through the Buwiz relay.
enum NetworkSession {
    Iaf(crate::transport::IafSftpSession),
    Buwiz(crate::relay::RelaySession),
}

impl SubmissionTransport for NetworkSubmissionTransport {
    type Session = NetworkSession;

    fn session_source(&self, session: &Self::Session) -> crate::transport::SftpEndpointSource {
        match session {
            NetworkSession::Iaf(session) => session.source(),
            NetworkSession::Buwiz(_) => crate::transport::SftpEndpointSource::Buwiz,
        }
    }

    fn open_session<'a>(
        &'a self,
        form_type: &'a str,
        tin: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Self::Session, crate::transport::TransportError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            if crate::transport::sends_through_buwiz() {
                return crate::relay::open().await.map(NetworkSession::Buwiz);
            }
            crate::transport::open_iaf_session(form_type, tin)
                .await
                .map(NetworkSession::Iaf)
        })
    }

    fn store_session<'a>(
        &'a self,
        session: Self::Session,
        filename: &'a str,
        payload: &'a [u8],
        xml: &'a [u8],
        note: bir_submit::Note<'a>,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        crate::transport::StoreReport,
                        crate::transport::TransportError,
                    >,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            match session {
                NetworkSession::Iaf(session) => session
                    .store(filename, payload)
                    .await
                    .map(|()| crate::transport::StoreReport::default()),
                NetworkSession::Buwiz(session) => session.store(filename, xml, note).await,
            }
        })
    }

    /// Only while sending goes through the Buwiz relay: a practice account, a dry run, the mock relay and SFTP
    /// never ask it anything.
    fn settle_pending<'a>(
        &'a self,
        pending: &'a str,
    ) -> Pin<Box<dyn Future<Output = crate::relay::Settlement> + Send + 'a>> {
        Box::pin(async move {
            if !crate::transport::sends_through_buwiz() {
                return crate::relay::Settlement::NotYet;
            }
            crate::relay::settle(pending).await
        })
    }
}

/// The worker's note of a Buwiz relay send ([`SubmissionTransport::store_session`]): the client's pending send, as
/// JSON, written by `write` on the claimed return. It fails when the claim is no longer this worker's.
fn pending_note<'a, W>(
    db: &'a Arc<Mutex<Database>>,
    write: W,
) -> impl Fn(&bir_submit::PendingSend) -> Result<(), String> + Send + Sync + 'a
where
    W: Fn(&Database, &str) -> Result<bool, crate::db::DbError> + Send + Sync + 'a,
{
    move |pending| {
        let json = serde_json::to_string(pending).map_err(|error| error.to_string())?;
        let db_guard =
            lock_timing::lock(db).map_err(|_| "the database is unavailable".to_owned())?;
        match write(&db_guard, &json) {
            Ok(true) => Ok(()),
            Ok(false) => Err("the claim on this return is no longer this worker's".to_owned()),
            Err(error) => Err(error.to_string()),
        }
    }
}

/// How long a noted send may stay without an answer from Buwiz before the person is asked instead.
const PENDING_PATIENCE: chrono::Duration = chrono::Duration::hours(24);

/// What the worker does with a claimed return whose noted relay send it did not see end, by what Buwiz says.
#[derive(Debug)]
enum Reconcile {
    /// A verified receipt: finish it as a send that went.
    Finish(crate::transport::StoreReport),
    /// No receipt: settle the claim as the same failure in this process would. An unknown outcome also drops the
    /// record: the server was asked and cannot say.
    Release(crate::transport::TransportError),
    /// Ask again on a later tick.
    Keep,
    /// Drop the record, and with it the waiting: the person answers (`needs_attention`).
    Clear,
}

fn reconcile_step(
    settlement: crate::relay::Settlement,
    pending: &str,
    now: chrono::DateTime<Utc>,
) -> Reconcile {
    use crate::relay::Settlement;
    match settlement {
        Settlement::Sent(report) => Reconcile::Finish(report),
        Settlement::Failed(error) => Reconcile::Release(error),
        Settlement::Gone => Reconcile::Clear,
        Settlement::NotYet => {
            let waited_long = serde_json::from_str::<bir_submit::PendingSend>(pending)
                .map_or(true, |pending| now - pending.noted_at >= PENDING_PATIENCE);
            if waited_long {
                Reconcile::Clear
            } else {
                Reconcile::Keep
            }
        }
    }
}

/// Of the XML this return may have been sent as, the one the noted send's hash names: what goes in
/// `submitted_payload`. `None` keeps what the return stores.
fn sent_payload(pending: &str, candidates: [Option<String>; 2]) -> Option<String> {
    use sha2::Digest;
    let wanted = serde_json::from_str::<bir_submit::PendingSend>(pending)
        .ok()?
        .xml_sha256;
    candidates
        .into_iter()
        .flatten()
        .find(|xml| hex::encode(sha2::Sha256::digest(xml.as_bytes())).eq_ignore_ascii_case(&wanted))
}

async fn process_queued_1601c(
    summary: &FormDraftSummary,
    profile: &TaxpayerProfile,
    db: Arc<Mutex<Database>>,
) {
    process_queued_1601c_with_transport(summary, profile, db, &NetworkSubmissionTransport).await;
}

async fn process_queued_1601c_with_transport<T: SubmissionTransport>(
    summary: &FormDraftSummary,
    profile: &TaxpayerProfile,
    db: Arc<Mutex<Database>>,
    transport: &T,
) {
    let Some(month) = summary.month else {
        warn!("Cron: Refusing 1601C queue row without a monthly period");
        return;
    };
    let loaded_draft = {
        let db_guard = match lock_timing::lock(&db) {
            Ok(guard) => guard,
            Err(_) => return,
        };
        match db_guard.get_1601c_draft(&summary.tin, summary.taxable_year, month) {
            Ok(Some(draft)) => draft,
            _ => return,
        }
    };

    let loaded_draft = if loaded_draft.submission_claim_token.is_some()
        || loaded_draft.submission_claimed_at.is_some()
    {
        // A relay send this worker noted and did not see end (the app was stopped during it): Buwiz knows.
        if let Some(pending) = loaded_draft.relay_pending.clone() {
            reconcile_1601c(loaded_draft, pending, profile, db, transport).await;
            return;
        }
        let recovered = {
            let db_guard = match lock_timing::lock(&db) {
                Ok(guard) => guard,
                Err(_) => return,
            };
            db_guard
                .recover_expired_unstarted_1601c_claim(
                    &loaded_draft.tin,
                    loaded_draft.taxable_year,
                    loaded_draft.month,
                    Utc::now(),
                )
                .unwrap_or(false)
        };
        if !recovered {
            warn!(
                "Cron: 1601C {} has an unresolved submission outcome; automatic retry is disabled and support-assisted manual reconciliation is required",
                loaded_draft.period_code()
            );
            crate::ipc::post_db_changed();
            return;
        }
        let recovered_draft = {
            let db_guard = match lock_timing::lock(&db) {
                Ok(guard) => guard,
                Err(_) => return,
            };
            match db_guard.get_1601c_draft(&summary.tin, summary.taxable_year, month) {
                Ok(Some(draft)) => draft,
                _ => return,
            }
        };
        if recovered_draft.submission_claim_token.is_some()
            || recovered_draft.submission_claimed_at.is_some()
        {
            return;
        }
        recovered_draft
    } else {
        loaded_draft
    };

    let (mut draft, queue_revision) = match prepare_queued_1601c(loaded_draft, None) {
        Queued1601CPreparation::Ready { draft, revision } => (draft, revision),
        Queued1601CPreparation::Rejected {
            draft,
            revision,
            errors,
        } => {
            let reason = validation_reason(&errors);
            warn!(
                "Cron: Refusing invalid queued 1601C {} before XML generation: {}",
                draft.period_code(),
                reason
            );
            if let Ok(db_guard) = lock_timing::lock(&db) {
                let _ = db_guard.replace_unclaimed_queued_1601c_submission(
                    &draft,
                    &revision.fingerprint,
                    &revision.next_retry_at,
                    revision.submission_attempts,
                );
            }
            crate::ipc::post_db_changed();
            return;
        }
        Queued1601CPreparation::Superseded => return,
    };

    match queued_retry_is_due(draft.next_retry_at.as_deref(), Utc::now()) {
        Ok(false) => return,
        Ok(true) => {}
        Err(error) => {
            warn!(
                "Cron: Refusing queued 1601C {} because its retry timestamp is invalid: {}",
                draft.period_code(),
                error
            );
            draft.revert_to_draft();
            draft.submission_error = Some(
                "Submission blocked because queued retry metadata is invalid; review and queue the return again"
                    .to_string(),
            );
            if let Ok(db_guard) = lock_timing::lock(&db) {
                let _ = db_guard.replace_unclaimed_queued_1601c_submission(
                    &draft,
                    &queue_revision.fingerprint,
                    &queue_revision.next_retry_at,
                    queue_revision.submission_attempts,
                );
            }
            crate::ipc::post_db_changed();
            return;
        }
    }

    info!(
        "Cron: Attempting to submit queued 1601C {} for {}",
        draft.period_code(),
        crate::logging::tin_for_log(&draft.tin)
    );
    let filename = draft.default_submission_filename();
    let xml_payload = match draft.try_to_bir_xml_payload() {
        Ok(payload) => payload,
        Err(errors) => {
            let reason = validation_reason(&errors);
            draft.revert_to_draft();
            draft.submission_error = Some(format!(
                "Submission blocked at XML generation boundary: {reason}"
            ));
            if let Ok(db_guard) = lock_timing::lock(&db) {
                let _ = db_guard.replace_unclaimed_queued_1601c_submission(
                    &draft,
                    &queue_revision.fingerprint,
                    &queue_revision.next_retry_at,
                    queue_revision.submission_attempts,
                );
            }
            crate::ipc::post_db_changed();
            return;
        }
    };
    let encrypted = match crate::crypto::compress_and_encrypt(
        xml_payload.as_bytes(),
        crate::crypto::BIR_IAF_PASSPHRASE,
    ) {
        Ok(encrypted) => encrypted,
        Err(error) => {
            fail_draft_1601c(&mut draft, &queue_revision, db.clone(), error.to_string());
            return;
        }
    };

    let Some(form_type) = crate::forms::queue_authorized_form_type_id("1601C") else {
        warn!(
            "Cron: Refusing to submit {} because 1601C is not authorized for queue submission",
            draft.period_code()
        );
        return;
    };

    let session = match transport.open_session(form_type, &draft.tin).await {
        Ok(session) => session,
        Err(error) => {
            settle_draft_1601c(
                &mut draft,
                &queue_revision,
                db.clone(),
                before_claim_note(&error),
                error.before_claim(),
            );
            crate::ipc::post_db_changed();
            return;
        }
    };

    // Claim only after the SFTP session is open, immediately before PUT.
    // Connect/login timeouts stay unclaimed and use the existing retry CAS.
    let claim_result = {
        let db_guard = match lock_timing::lock(&db) {
            Ok(guard) => guard,
            Err(_) => return,
        };
        db_guard.claim_queued_1601c_submission(
            &draft.tin,
            draft.taxable_year,
            draft.month,
            &queue_revision.fingerprint,
            &queue_revision.next_retry_at,
            queue_revision.submission_attempts,
        )
    };
    let claim_token = match claim_result {
        Ok(Claim1601CSubmissionResult::Claimed {
            draft: claimed_draft,
            token,
        }) => {
            draft = claimed_draft;
            crate::ipc::post_db_changed();
            token
        }
        Ok(Claim1601CSubmissionResult::Rejected { draft, errors }) => {
            warn!(
                "Cron: 1601C submission claim rejected {}: {}",
                draft.period_code(),
                validation_reason(&errors)
            );
            crate::ipc::post_db_changed();
            return;
        }
        Ok(Claim1601CSubmissionResult::Superseded) => {
            info!(
                "Cron: 1601C submission job for {} was canceled or superseded before PUT",
                draft.period_code()
            );
            return;
        }
        Err(error) => {
            warn!(
                "Cron: Could not claim queued 1601C {} because the database claim failed: {}",
                draft.period_code(),
                error
            );
            return;
        }
    };

    let session_source = transport.session_source(&session);
    let put_marked = {
        let db_guard = match lock_timing::lock(&db) {
            Ok(guard) => guard,
            Err(_) => return,
        };
        db_guard
            .mark_claimed_1601c_put_started(
                &draft.tin,
                draft.taxable_year,
                draft.month,
                &claim_token,
            )
            .unwrap_or(false)
    };
    if !put_marked {
        warn!(
            "Cron: 1601C {} claim disappeared before PUT start was recorded; skipping PUT",
            draft.period_code()
        );
        return;
    }
    let (tin, year, month) = (draft.tin.clone(), draft.taxable_year, draft.month);
    let note = pending_note(&db, |db_guard, pending| {
        db_guard.note_claimed_1601c_relay_pending(&tin, year, month, &claim_token, Some(pending))
    });
    match transport
        .store_session(
            session,
            &filename,
            &encrypted,
            xml_payload.as_bytes(),
            &note,
        )
        .await
    {
        Ok(report) => finish_sent_1601c(
            draft,
            profile,
            &db,
            &claim_token,
            filename,
            Some(xml_payload),
            session_source,
            report,
        ),
        Err(error) => {
            release_unaccepted(
                "1601C",
                &draft.period_code(),
                &error,
                &db,
                |db_guard, note, how| {
                    db_guard.release_unaccepted_claimed_1601c_submission(
                        &tin,
                        year,
                        month,
                        &claim_token,
                        note,
                        how,
                    )
                },
            );
        }
    }
}

// ===== Generic queued submission for every `QueueableForm` =====

/// The queue generation a worker loaded; every CAS names it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct QueueGeneration {
    fingerprint: Option<String>,
    next_retry_at: Option<String>,
    submission_attempts: u32,
}

fn queue_generation<F: QueueableForm>(draft: &F) -> Option<QueueGeneration> {
    let lifecycle = draft.lifecycle();
    (matches!(lifecycle.status, FilingStatus::Queued) && lifecycle.is_unclaimed()).then(|| {
        QueueGeneration {
            fingerprint: lifecycle.queued_submission_fingerprint.clone(),
            next_retry_at: lifecycle.next_retry_at.clone(),
            submission_attempts: lifecycle.submission_attempts,
        }
    })
}

/// Persist an unclaimed generation update (retry state or a return to Draft).
fn replace_queueable<F: QueueableForm>(
    db: &Arc<Mutex<Database>>,
    draft: &F,
    generation: &QueueGeneration,
) {
    if let Ok(db_guard) = lock_timing::lock(db) {
        match db_guard.replace_unclaimed_queued_queueable(
            draft,
            &generation.fingerprint,
            &generation.next_retry_at,
            generation.submission_attempts,
        ) {
            Ok(true) => {}
            Ok(false) => info!(
                "Cron: {} {} was canceled or superseded before its state persisted",
                F::FORM_CODE,
                draft.period_code()
            ),
            Err(error) => warn!(
                "Cron: Failed to persist {} {} queue state: {}",
                F::FORM_CODE,
                draft.period_code(),
                error
            ),
        }
    }
}

fn fail_queueable<F: QueueableForm>(
    draft: &mut F,
    generation: &QueueGeneration,
    db: &Arc<Mutex<Database>>,
    error_message: String,
) {
    settle_queueable(draft, generation, db, error_message, NoReceipt::Failed);
}

/// A send that ended before any claim: a failed try, or a hold that is not one.
fn settle_queueable<F: QueueableForm>(
    draft: &mut F,
    generation: &QueueGeneration,
    db: &Arc<Mutex<Database>>,
    error_message: String,
    how: NoReceipt,
) {
    warn!(
        "Cron: {} submission preparation failed for {}: {}",
        F::FORM_CODE,
        draft.period_code(),
        error_message
    );
    draft
        .lifecycle_mut()
        .settle_without_receipt(error_message, how);
    replace_queueable(db, draft, generation);
}

/// The note kept on a return whose send ended before any claim. A hold says its own sentence (the person reads
/// it); anything else is the SFTP session failing, as before.
fn before_claim_note(error: &crate::transport::TransportError) -> String {
    match error {
        crate::transport::TransportError::RelayHeld { reason, .. } => reason.clone(),
        _ => format!("BIR SFTP session failed before upload (no PUT attempted): {error}"),
    }
}

/// Human-readable period for notices: `09/26`, `Q3/26` or `2026`.
fn queueable_period_label<F: QueueableForm>(draft: &F) -> String {
    match draft.filing_period() {
        crate::forms::FilingPeriod::Monthly(month) => {
            monthly_period_label(draft.taxable_year(), month)
        }
        crate::forms::FilingPeriod::Quarterly(quarter) => {
            quarterly_period_label(draft.taxable_year(), quarter)
        }
        _ => draft.taxable_year().to_string(),
    }
}

/// `authorized_form_type` is [`crate::forms::queue_authorized_form_type_id`]
/// in production: no PUT happens unless the capability registry allows it.
async fn process_queued_queueable<F: QueueableForm, T: SubmissionTransport>(
    summary: &FormDraftSummary,
    profile: &TaxpayerProfile,
    db: Arc<Mutex<Database>>,
    transport: &T,
    authorized_form_type: fn(&str) -> Option<&'static str>,
) {
    let period = i64::from(summary.month.or(summary.quarter).unwrap_or(0));
    let load = |db: &Arc<Mutex<Database>>| -> Option<F> {
        let db_guard = lock_timing::lock(db).ok()?;
        db_guard
            .get_queueable_draft::<F>(&summary.tin, summary.taxable_year, period)
            .ok()
            .flatten()
    };
    let Some(loaded) = load(&db) else {
        return;
    };

    // A held claim is only recoverable when its lease expired before PUT.
    let loaded = if !loaded.lifecycle().is_unclaimed() {
        // A relay send this worker noted and did not see end (the app was stopped during it): Buwiz knows.
        if let Some(pending) = loaded.lifecycle().relay_pending.clone() {
            reconcile_queueable(loaded, pending, profile, db, transport).await;
            return;
        }
        let recovered = lock_timing::lock(&db)
            .ok()
            .and_then(|db_guard| {
                db_guard
                    .recover_expired_unstarted_queueable_claim::<F>(
                        &summary.tin,
                        summary.taxable_year,
                        period,
                        Utc::now(),
                    )
                    .ok()
            })
            .unwrap_or(false);
        if !recovered {
            warn!(
                "Cron: {} {} has an unresolved submission outcome; automatic retry is disabled and support-assisted manual reconciliation is required",
                F::FORM_CODE,
                loaded.period_code()
            );
            crate::ipc::post_db_changed();
            return;
        }
        match load(&db) {
            Some(draft) if draft.lifecycle().is_unclaimed() => draft,
            _ => return,
        }
    } else {
        loaded
    };

    let Some(generation) = queue_generation(&loaded) else {
        return;
    };
    let mut draft = loaded;
    if let Err(errors) = draft.revalidate_queued_before_submission() {
        warn!(
            "Cron: Refusing invalid queued {} {} before XML generation: {}",
            F::FORM_CODE,
            draft.period_code(),
            validation_reason(&errors)
        );
        replace_queueable(&db, &draft, &generation);
        crate::ipc::post_db_changed();
        return;
    }

    match queued_retry_is_due(draft.lifecycle().next_retry_at.as_deref(), Utc::now()) {
        Ok(false) => return,
        Ok(true) => {}
        Err(error) => {
            warn!(
                "Cron: Refusing queued {} {} because its retry timestamp is invalid: {}",
                F::FORM_CODE,
                draft.period_code(),
                error
            );
            draft.lifecycle_mut().revert_to_draft();
            draft.lifecycle_mut().submission_error = Some(
                "Submission blocked because queued retry metadata is invalid; review and queue the return again"
                    .to_string(),
            );
            replace_queueable(&db, &draft, &generation);
            crate::ipc::post_db_changed();
            return;
        }
    }

    info!(
        "Cron: Attempting to submit queued {} {} for {}",
        F::FORM_CODE,
        draft.period_code(),
        crate::logging::tin_for_log(draft.tin())
    );
    let filename = draft.submission_filename();
    let payload = match draft.official_payload() {
        Ok(payload) => payload,
        Err(errors) => {
            let reason = validation_reason(&errors);
            draft.lifecycle_mut().revert_to_draft();
            draft.lifecycle_mut().submission_error = Some(format!(
                "Submission blocked at XML generation boundary: {reason}"
            ));
            replace_queueable(&db, &draft, &generation);
            crate::ipc::post_db_changed();
            return;
        }
    };
    let encrypted = match crate::crypto::compress_and_encrypt(
        payload.as_bytes(),
        crate::crypto::BIR_IAF_PASSPHRASE,
    ) {
        Ok(encrypted) => encrypted,
        Err(error) => {
            fail_queueable(&mut draft, &generation, &db, error.to_string());
            return;
        }
    };

    let Some(form_type) = authorized_form_type(F::FORM_CODE) else {
        warn!(
            "Cron: Refusing to submit {} because {} is not authorized for queue submission",
            draft.period_code(),
            F::FORM_CODE
        );
        return;
    };

    let session = match transport.open_session(form_type, draft.tin()).await {
        Ok(session) => session,
        Err(error) => {
            settle_queueable(
                &mut draft,
                &generation,
                &db,
                before_claim_note(&error),
                error.before_claim(),
            );
            crate::ipc::post_db_changed();
            return;
        }
    };

    // Claim only after the session is open, immediately before PUT.
    let claim = {
        let Ok(db_guard) = lock_timing::lock(&db) else {
            return;
        };
        db_guard.claim_queued_queueable::<F>(
            draft.tin(),
            draft.taxable_year(),
            draft.period_column(),
            &generation.fingerprint,
            &generation.next_retry_at,
            generation.submission_attempts,
        )
    };
    let claim_token = match claim {
        Ok(crate::db::ClaimQueueableResult::Claimed {
            draft: claimed,
            token,
        }) => {
            draft = claimed;
            crate::ipc::post_db_changed();
            token
        }
        Ok(crate::db::ClaimQueueableResult::Rejected { draft, errors }) => {
            warn!(
                "Cron: {} submission claim rejected {}: {}",
                F::FORM_CODE,
                draft.period_code(),
                validation_reason(&errors)
            );
            crate::ipc::post_db_changed();
            return;
        }
        Ok(crate::db::ClaimQueueableResult::Superseded) => {
            info!(
                "Cron: {} submission job for {} was canceled or superseded before PUT",
                F::FORM_CODE,
                draft.period_code()
            );
            return;
        }
        Err(error) => {
            warn!(
                "Cron: Could not claim queued {} {} because the database claim failed: {}",
                F::FORM_CODE,
                draft.period_code(),
                error
            );
            return;
        }
    };

    let session_source = transport.session_source(&session);
    let put_marked = lock_timing::lock(&db)
        .ok()
        .and_then(|db_guard| {
            db_guard
                .mark_claimed_queueable_put_started::<F>(
                    draft.tin(),
                    draft.taxable_year(),
                    draft.period_column(),
                    &claim_token,
                )
                .ok()
        })
        .unwrap_or(false);
    if !put_marked {
        warn!(
            "Cron: {} {} claim disappeared before PUT start was recorded; skipping PUT",
            F::FORM_CODE,
            draft.period_code()
        );
        return;
    }

    let (tin, year, period) = (
        draft.tin().to_string(),
        draft.taxable_year(),
        draft.period_column(),
    );
    let note = pending_note(&db, |db_guard, pending| {
        db_guard.note_claimed_queueable_relay_pending::<F>(
            &tin,
            year,
            period,
            &claim_token,
            Some(pending),
        )
    });
    match transport
        .store_session(session, &filename, &encrypted, payload.as_bytes(), &note)
        .await
    {
        Ok(report) => finish_sent_queueable(
            draft,
            profile,
            &db,
            &claim_token,
            filename,
            Some(payload),
            session_source,
            report,
        ),
        Err(error) => {
            release_unaccepted(
                F::FORM_CODE,
                &draft.period_code(),
                &error,
                &db,
                |db_guard, note, how| {
                    db_guard.release_unaccepted_claimed_queueable::<F>(
                        &tin,
                        year,
                        period,
                        &claim_token,
                        note,
                        how,
                    )
                },
            );
        }
    }
}

/// A queued return whose send went: Submitted, with the relay's receipt, the notice and the alert, and the inbox
/// watched for the BIR's email unless nothing reached the BIR. `payload` is what was sent (`None` keeps the stored
/// one).
#[allow(clippy::too_many_arguments)]
fn finish_sent_queueable<F: QueueableForm>(
    mut draft: F,
    profile: &TaxpayerProfile,
    db: &Arc<Mutex<Database>>,
    claim_token: &str,
    filename: String,
    payload: Option<String>,
    session_source: crate::transport::SftpEndpointSource,
    report: crate::transport::StoreReport,
) {
    let skip_email_poll =
        session_source.is_dry_run() || session_source.is_relay() || report.simulated;
    if skip_email_poll {
        info!(
            "Cron: dry run: simulated submission of queued {} {} (nothing sent to BIR)",
            F::FORM_CODE,
            crate::logging::submission_filename_for_log(filename.as_str())
        );
    } else {
        info!(
            "Cron: Successfully submitted queued {} {}",
            F::FORM_CODE,
            crate::logging::submission_filename_for_log(filename.as_str())
        );
    }
    let period_label = queueable_period_label(&draft);
    let (notice_title, notice_body) = submission_alert_text(
        profile,
        F::FORM_CODE,
        &period_label,
        &chrono::Local::now(),
        skip_email_poll,
    );
    crate::notification::notify(crate::notification::NotificationKind::ReturnSent {
        dry_run: skip_email_poll,
    });
    let submitted_file = filename.clone();
    // Keep the exact bytes that were encrypted for PUT, so an
    // amendment can later snapshot what BIR received.
    if let Some(payload) = payload {
        draft.lifecycle_mut().submitted_payload = Some(payload);
    }
    draft
        .lifecycle_mut()
        .transition_to_submitted(filename, session_source.is_dry_run() || report.simulated);
    draft.lifecycle_mut().submitted_via_relay = session_source.is_relay();
    draft.lifecycle_mut().relay_receipt = report.relay_receipt;
    if let Ok(db_guard) = lock_timing::lock(db) {
        match db_guard.finish_claimed_queueable_submission(&draft, claim_token) {
            Ok(_) => {
                record_submission_alert(
                    &db_guard,
                    profile,
                    F::FORM_CODE,
                    &period_label,
                    &submitted_file,
                    &notice_title,
                    &notice_body,
                );
                if !skip_email_poll {
                    let (month, quarter) = match draft.filing_period() {
                        crate::forms::FilingPeriod::Monthly(month) => (Some(month), None),
                        crate::forms::FilingPeriod::Quarterly(quarter) => (None, Some(quarter)),
                        _ => (None, None),
                    };
                    schedule_email_poll(
                        profile,
                        F::FORM_CODE,
                        draft.tin(),
                        draft.taxable_year(),
                        month,
                        quarter,
                        &db_guard,
                    );
                }
                crate::dev_mode::maybe_auto_confirm(&db_guard, session_source, &submitted_file);
            }
            Err(error) => warn!(
                "Cron: {} {} was transmitted but its submission claim could not be finalized: {}",
                F::FORM_CODE,
                draft.period_code(),
                error
            ),
        }
    }
}

/// A claimed queued return whose noted relay send this worker did not see end: ask Buwiz and act on what it says.
async fn reconcile_queueable<F: QueueableForm, T: SubmissionTransport>(
    draft: F,
    pending: String,
    profile: &TaxpayerProfile,
    db: Arc<Mutex<Database>>,
    transport: &T,
) {
    let Some(claim_token) = draft.lifecycle().submission_claim_token.clone() else {
        return;
    };
    let (tin, year, period) = (
        draft.tin().to_string(),
        draft.taxable_year(),
        draft.period_column(),
    );
    let clear = |db: &Arc<Mutex<Database>>| {
        if let Ok(db_guard) = lock_timing::lock(db) {
            let _ = db_guard.note_claimed_queueable_relay_pending::<F>(
                &tin,
                year,
                period,
                &claim_token,
                None,
            );
        }
    };
    match reconcile_step(
        transport.settle_pending(&pending).await,
        &pending,
        Utc::now(),
    ) {
        Reconcile::Keep => return,
        Reconcile::Clear => clear(&db),
        Reconcile::Finish(report) => {
            let filename = draft.submission_filename();
            let payload = sent_payload(
                &pending,
                [
                    draft.official_payload().ok(),
                    draft.lifecycle().submitted_payload.clone(),
                ],
            );
            finish_sent_queueable(
                draft,
                profile,
                &db,
                &claim_token,
                filename,
                payload,
                crate::transport::SftpEndpointSource::Buwiz,
                report,
            );
        }
        Reconcile::Release(error) => {
            release_unaccepted(
                F::FORM_CODE,
                &draft.period_code(),
                &error,
                &db,
                |db_guard, note, how| {
                    db_guard.release_unaccepted_claimed_queueable::<F>(
                        &tin,
                        year,
                        period,
                        &claim_token,
                        note,
                        how,
                    )
                },
            );
            if matches!(error.no_receipt(), NoReceipt::Unknown) {
                clear(&db);
            }
        }
    }
    crate::ipc::post_db_changed();
}

/// Run the generic worker for a queueable form code. `false` when the code
/// is not a [`QueueableKind`].
async fn process_queued_by_kind(
    summary: &FormDraftSummary,
    profile: &TaxpayerProfile,
    db: Arc<Mutex<Database>>,
) -> bool {
    let Some(kind) = QueueableKind::from_form_code(&summary.form_code) else {
        return false;
    };
    crate::with_queueable_kind!(kind, F => {
        process_queued_queueable::<F, _>(
            summary,
            profile,
            db,
            &NetworkSubmissionTransport,
            crate::forms::queue_authorized_form_type_id,
        )
        .await
    });
    true
}

#[cfg(test)]
fn submission_queue_year_at<Tz: chrono::TimeZone>(now: &chrono::DateTime<Tz>) -> u16 {
    use chrono::Datelike;
    now.year() as u16
}

async fn process_queued_2551q(
    summary: &FormDraftSummary,
    profile: &TaxpayerProfile,
    db: Arc<Mutex<Database>>,
) {
    process_queued_2551q_with_transport(summary, profile, db, &NetworkSubmissionTransport).await;
}

/// One queued 2551Q: revalidate, claim, PUT through `transport`, finish.
async fn process_queued_2551q_with_transport<T: SubmissionTransport>(
    summary: &FormDraftSummary,
    profile_clone: &TaxpayerProfile,
    db_clone: Arc<Mutex<Database>>,
    transport: &T,
) {
    let (loaded_draft, current_profile) = {
        let db_guard = match lock_timing::lock(&db_clone) {
            Ok(g) => g,
            Err(_) => return,
        };
        let current_profile =
            match db_guard.get_profile_for_year(&profile_clone.tin.full(), summary.taxable_year) {
                Ok(Some(profile)) => profile,
                _ => return,
            };
        let draft = match db_guard.get_2551q_draft(
            &current_profile.tin.full(),
            summary.taxable_year,
            summary.quarter.unwrap_or(0),
        ) {
            Ok(Some(draft)) => draft,
            _ => return,
        };
        (draft, current_profile)
    };

    if loaded_draft.submission_claim_token.is_some() || loaded_draft.submission_claimed_at.is_some()
    {
        // A relay send this worker noted and did not see end (the app was stopped during it): Buwiz knows.
        if let Some(pending) = loaded_draft.relay_pending.clone() {
            reconcile_2551q(loaded_draft, pending, &current_profile, db_clone, transport).await;
            return;
        }
        let recovered = {
            let db_guard = match lock_timing::lock(&db_clone) {
                Ok(guard) => guard,
                Err(_) => return,
            };
            db_guard
                .recover_expired_unstarted_2551q_claim(
                    &loaded_draft.tin,
                    loaded_draft.taxable_year,
                    loaded_draft.quarter,
                    Utc::now(),
                )
                .unwrap_or(false)
        };
        if !recovered {
            warn!(
                "Cron: Form {} has an unresolved submission outcome; automatic retry is disabled and support-assisted manual reconciliation is required",
                loaded_draft.period_code()
            );
            crate::ipc::post_db_changed();
            return;
        }
        return;
    }

    let (mut draft, queue_revision) =
        match prepare_queued_2551q(loaded_draft, &current_profile, None) {
            Queued2551QPreparation::Ready { draft, revision } => (draft, revision),
            Queued2551QPreparation::Rejected {
                draft,
                revision,
                errors,
            } => {
                let reason = validation_reason(&errors);
                warn!(
                    "Cron: Refusing invalid queued form {} before XML generation: {}",
                    draft.period_code(),
                    reason
                );
                if let Ok(db_guard) = lock_timing::lock(&db_clone) {
                    let _ = db_guard.replace_unclaimed_queued_2551q_submission(
                        &draft,
                        &revision.fingerprint,
                        &revision.next_retry_at,
                        revision.submission_attempts,
                    );
                }
                crate::ipc::post_db_changed();
                return;
            }
            Queued2551QPreparation::Superseded => return,
        };

    match queued_retry_is_due(draft.next_retry_at.as_deref(), Utc::now()) {
        Ok(false) => return,
        Ok(true) => {}
        Err(error) => {
            warn!(
                "Cron: Refusing queued form {} because its retry timestamp is invalid: {}",
                draft.period_code(),
                error
            );
            draft.revert_to_draft();
            draft.last_error = Some(
                "Submission blocked because queued retry metadata is invalid; review and queue the return again"
                    .to_string(),
            );
            if let Ok(db_guard) = lock_timing::lock(&db_clone) {
                let _ = db_guard.replace_unclaimed_queued_2551q_submission(
                    &draft,
                    &queue_revision.fingerprint,
                    &queue_revision.next_retry_at,
                    queue_revision.submission_attempts,
                );
            }
            crate::ipc::post_db_changed();
            return;
        }
    }

    info!(
        "Cron: Attempting to submit queued form {} for {}",
        draft.period_code(),
        crate::logging::tin_for_log(&current_profile.tin.full())
    );

    let filename = draft.default_submission_filename();
    let xml_payload = match draft.to_bir_xml_payload() {
        Ok(payload) => payload,
        Err(errors) => {
            let reason = errors
                .iter()
                .map(|(field, message)| format!("{field}: {message}"))
                .collect::<Vec<_>>()
                .join("; ");
            warn!(
                "Cron: Refusing invalid form {} at XML generation boundary: {}",
                draft.period_code(),
                reason
            );
            draft.revert_to_draft();
            draft.last_error = Some(format!(
                "Submission blocked at XML generation boundary: {reason}"
            ));
            if let Ok(db_guard) = lock_timing::lock(&db_clone) {
                let _ = db_guard.replace_unclaimed_queued_2551q_submission(
                    &draft,
                    &queue_revision.fingerprint,
                    &queue_revision.next_retry_at,
                    queue_revision.submission_attempts,
                );
            }
            crate::ipc::post_db_changed();
            return;
        }
    };
    let encrypted = match crate::crypto::compress_and_encrypt(
        xml_payload.as_bytes(),
        crate::crypto::BIR_IAF_PASSPHRASE,
    ) {
        Ok(enc) => enc,
        Err(e) => {
            fail_draft_2551q(&mut draft, &queue_revision, db_clone.clone(), e.to_string());
            return;
        }
    };

    // Resolve the exact transport identifier from the audited
    // capability registry at the irreversible boundary. Never
    // revive an uncertified submitter with a hard-coded fallback.
    let Some(form_type) = crate::forms::queue_authorized_form_type_id("2551Q") else {
        warn!(
            "Cron: Refusing to submit {} because 2551Q is not authorized for queue submission",
            draft.period_code()
        );
        return;
    };

    let session = match transport.open_session(form_type, &draft.tin).await {
        Ok(session) => session,
        Err(error) => {
            settle_draft_2551q(
                &mut draft,
                &queue_revision,
                db_clone.clone(),
                before_claim_note(&error),
                error.before_claim(),
            );
            crate::ipc::post_db_changed();
            return;
        }
    };

    // Claim only after the SFTP session is open, immediately before PUT.
    let claim_result = {
        let db_guard = match lock_timing::lock(&db_clone) {
            Ok(guard) => guard,
            Err(_) => return,
        };
        db_guard.claim_queued_2551q_submission(
            &draft.tin,
            draft.taxable_year,
            draft.quarter,
            &queue_revision.fingerprint,
            &queue_revision.next_retry_at,
            queue_revision.submission_attempts,
        )
    };
    let claim_token = match claim_result {
        Ok(Claim2551QSubmissionResult::Claimed {
            draft: claimed_draft,
            token,
        }) => {
            draft = claimed_draft;
            // Let an open form window replace its stale Queued copy
            // with the claimed row before PUT returns.
            crate::ipc::post_db_changed();
            token
        }
        Ok(Claim2551QSubmissionResult::Rejected {
            draft: rejected_draft,
            errors,
        }) => {
            let reason = validation_reason(&errors);
            warn!(
                "Cron: Submission claim rejected form {}: {}",
                rejected_draft.period_code(),
                reason
            );
            crate::ipc::post_db_changed();
            return;
        }
        Ok(Claim2551QSubmissionResult::Superseded) => {
            info!(
                "Cron: Submission job for {} was canceled or superseded before PUT",
                draft.period_code()
            );
            return;
        }
        Err(_) => {
            warn!(
                "Cron: Could not claim queued form {} because the database claim failed",
                draft.period_code()
            );
            return;
        }
    };

    let session_source = transport.session_source(&session);
    let put_marked = {
        let db_guard = match lock_timing::lock(&db_clone) {
            Ok(guard) => guard,
            Err(_) => return,
        };
        db_guard
            .mark_claimed_2551q_put_started(
                &draft.tin,
                draft.taxable_year,
                draft.quarter,
                &claim_token,
            )
            .unwrap_or(false)
    };
    if !put_marked {
        warn!(
            "Cron: Form {} claim disappeared before PUT start was recorded; skipping PUT",
            draft.period_code()
        );
        return;
    }
    let (tin, year, quarter) = (draft.tin.clone(), draft.taxable_year, draft.quarter);
    let note = pending_note(&db_clone, |db_guard, pending| {
        db_guard.note_claimed_2551q_relay_pending(&tin, year, quarter, &claim_token, Some(pending))
    });
    match transport
        .store_session(
            session,
            &filename,
            &encrypted,
            xml_payload.as_bytes(),
            &note,
        )
        .await
    {
        Ok(report) => finish_sent_2551q(
            draft,
            &current_profile,
            &db_clone,
            &claim_token,
            filename,
            Some(xml_payload),
            session_source,
            report,
        ),
        Err(error) => {
            release_unaccepted(
                "2551Q",
                &draft.period_code(),
                &error,
                &db_clone,
                |db_guard, note, how| {
                    db_guard.release_unaccepted_claimed_2551q_submission(
                        &tin,
                        year,
                        quarter,
                        &claim_token,
                        note,
                        how,
                    )
                },
            );
        }
    }
    crate::ipc::post_db_changed();
}

/// A 2551Q whose send went: Submitted, with the relay's receipt, the notice and the alert, and the inbox watched for
/// the BIR's email unless nothing reached the BIR. `xml_payload` is what was sent (`None` keeps the stored one).
#[allow(clippy::too_many_arguments)]
fn finish_sent_2551q(
    mut draft: Form2551QDraft,
    current_profile: &TaxpayerProfile,
    db: &Arc<Mutex<Database>>,
    claim_token: &str,
    filename: String,
    xml_payload: Option<String>,
    session_source: crate::transport::SftpEndpointSource,
    report: crate::transport::StoreReport,
) {
    let skip_email_poll =
        session_source.is_dry_run() || session_source.is_relay() || report.simulated;
    if skip_email_poll {
        info!(
            "Cron: dry run: simulated submission of queued form {} (nothing sent to BIR)",
            crate::logging::submission_filename_for_log(filename.as_str())
        );
    } else {
        info!(
            "Cron: Successfully submitted queued form {}",
            crate::logging::submission_filename_for_log(filename.as_str())
        );
    }
    let period_label = quarterly_period_label(draft.taxable_year, draft.quarter);
    let (notice_title, notice_body) = submission_alert_text(
        current_profile,
        "2551Q",
        &period_label,
        &chrono::Local::now(),
        skip_email_poll,
    );
    crate::notification::notify(crate::notification::NotificationKind::ReturnSent {
        dry_run: skip_email_poll,
    });
    // Keep the exact bytes that were encrypted for PUT.
    if let Some(xml_payload) = xml_payload {
        draft.submitted_payload = Some(xml_payload);
    }
    draft.transition_to_submitted(
        filename.clone(),
        session_source.is_dry_run() || report.simulated,
    );
    draft.submitted_via_relay = session_source.is_relay();
    draft.relay_receipt = report.relay_receipt;
    if let Ok(db_guard) = lock_timing::lock(db) {
        match db_guard.finish_claimed_2551q_submission(&draft, claim_token) {
            Ok(_) => {
                record_submission_alert(
                    &db_guard,
                    current_profile,
                    "2551Q",
                    &period_label,
                    &filename,
                    &notice_title,
                    &notice_body,
                );
                if skip_email_poll {
                    info!("Cron: 2551Q dry-run PUT succeeded; skipping IMAP poll (source=dry-run)");
                } else {
                    schedule_email_poll(
                        current_profile,
                        "2551Q",
                        &draft.tin,
                        draft.taxable_year,
                        None,
                        Some(draft.quarter),
                        &db_guard,
                    );
                }
                crate::dev_mode::maybe_auto_confirm(&db_guard, session_source, &filename);
            }
            Err(error) => warn!(
                "Cron: Form {} was transmitted but its submission claim could not be finalized: {}",
                draft.period_code(),
                error
            ),
        }
    }
}

/// A claimed 2551Q whose noted relay send this worker did not see end: ask Buwiz and act on what it says.
async fn reconcile_2551q<T: SubmissionTransport>(
    draft: Form2551QDraft,
    pending: String,
    current_profile: &TaxpayerProfile,
    db: Arc<Mutex<Database>>,
    transport: &T,
) {
    let Some(claim_token) = draft.submission_claim_token.clone() else {
        return;
    };
    let (tin, year, quarter) = (draft.tin.clone(), draft.taxable_year, draft.quarter);
    let clear = |db: &Arc<Mutex<Database>>| {
        if let Ok(db_guard) = lock_timing::lock(db) {
            let _ =
                db_guard.note_claimed_2551q_relay_pending(&tin, year, quarter, &claim_token, None);
        }
    };
    match reconcile_step(
        transport.settle_pending(&pending).await,
        &pending,
        Utc::now(),
    ) {
        Reconcile::Keep => return,
        Reconcile::Clear => clear(&db),
        Reconcile::Finish(report) => {
            let filename = draft.default_submission_filename();
            let payload = sent_payload(
                &pending,
                [
                    draft.to_bir_xml_payload().ok(),
                    draft.submitted_payload.clone(),
                ],
            );
            finish_sent_2551q(
                draft,
                current_profile,
                &db,
                &claim_token,
                filename,
                payload,
                crate::transport::SftpEndpointSource::Buwiz,
                report,
            );
        }
        Reconcile::Release(error) => {
            release_unaccepted(
                "2551Q",
                &draft.period_code(),
                &error,
                &db,
                |db_guard, note, how| {
                    db_guard.release_unaccepted_claimed_2551q_submission(
                        &tin,
                        year,
                        quarter,
                        &claim_token,
                        note,
                        how,
                    )
                },
            );
            if matches!(error.no_receipt(), NoReceipt::Unknown) {
                clear(&db);
            }
        }
    }
    crate::ipc::post_db_changed();
}

/// The setting that says the returns queued before this database first met a version that sends through Buwiz
/// were given back to their owner ([`give_back_returns_queued_before_the_relay`]).
pub const RELAY_SINCE_SETTING: &str = "relay_filing_since";

/// The reason on a return that was queued before the app could send through Buwiz.
pub const QUEUED_BEFORE_RELAY: &str = "This return was queued in an earlier version of eBIRForms and was not sent. Check it and queue it again to send it to the BIR through Buwiz.";

/// Once per database, the first time a version that files through Buwiz runs its queue: every return that is
/// still Queued and unclaimed goes back to Draft with [`QUEUED_BEFORE_RELAY`]. The person queued it for another
/// way of sending, perhaps long ago; sending it now, unasked, would be filing by accident. A claimed return is
/// left as it is: its outcome is unknown and the person settles it. Returns queued after this are sent as usual.
fn give_back_returns_queued_before_the_relay(db: &Arc<Mutex<Database>>) {
    let queued = {
        let Ok(db_guard) = lock_timing::lock(db) else {
            return;
        };
        match db_guard.get_setting(RELAY_SINCE_SETTING) {
            Ok(None) => {}
            // Done before, or the setting cannot be read: never sweep twice.
            _ => return,
        }
        let queued = db_guard.list_all_queued_submissions().unwrap_or_default();
        if let Err(error) = db_guard.set_setting(RELAY_SINCE_SETTING, &Utc::now().to_rfc3339()) {
            warn!("Cron: could not record the first run with relay filing: {error}");
            return;
        }
        queued
    };
    for summary in queued
        .iter()
        .filter(|summary| summary.status == FilingStatus::Queued)
    {
        give_back_one(summary, db);
    }
    crate::ipc::post_db_changed();
}

fn give_back_one(summary: &FormDraftSummary, db: &Arc<Mutex<Database>>) {
    let note = QUEUED_BEFORE_RELAY.to_string();
    match summary.form_code.as_str() {
        "2551Q" => {
            let Some(quarter) = summary.quarter else {
                return;
            };
            let loaded = lock_timing::lock(db).ok().and_then(|db_guard| {
                db_guard
                    .get_2551q_draft(&summary.tin, summary.taxable_year, quarter)
                    .ok()
                    .flatten()
            });
            if let Some(mut draft) = loaded
                && let Some(revision) = queued_2551q_revision(&draft)
            {
                settle_draft_2551q(&mut draft, &revision, db.clone(), note, NoReceipt::Refused);
            }
        }
        "1601C" => {
            let Some(month) = summary.month else {
                return;
            };
            let loaded = lock_timing::lock(db).ok().and_then(|db_guard| {
                db_guard
                    .get_1601c_draft(&summary.tin, summary.taxable_year, month)
                    .ok()
                    .flatten()
            });
            if let Some(mut draft) = loaded
                && let Some(revision) = queued_1601c_revision(&draft)
            {
                settle_draft_1601c(&mut draft, &revision, db.clone(), note, NoReceipt::Refused);
            }
        }
        code => {
            let Some(kind) = QueueableKind::from_form_code(code) else {
                return;
            };
            crate::with_queueable_kind!(kind, F => give_back_queueable::<F>(summary, db, note));
        }
    }
}

fn give_back_queueable<F: QueueableForm>(
    summary: &FormDraftSummary,
    db: &Arc<Mutex<Database>>,
    note: String,
) {
    let period = i64::from(summary.month.or(summary.quarter).unwrap_or(0));
    let loaded = lock_timing::lock(db).ok().and_then(|db_guard| {
        db_guard
            .get_queueable_draft::<F>(&summary.tin, summary.taxable_year, period)
            .ok()
            .flatten()
    });
    if let Some(mut draft) = loaded
        && let Some(generation) = queue_generation(&draft)
    {
        settle_queueable(&mut draft, &generation, db, note, NoReceipt::Refused);
    }
}

async fn process_submission_queue(profile: &TaxpayerProfile, db: Arc<Mutex<Database>>) {
    // A build with nowhere to send leaves every queued return as it is: no
    // attempt is counted and none goes back to Draft.
    if crate::transport::sending_unavailable() {
        return;
    }
    // The first time this database meets a version that files through Buwiz, what was queued before is given back.
    if crate::transport::sends_through_buwiz() {
        give_back_returns_queued_before_the_relay(&db);
    }
    let summaries = {
        let db_guard = match lock_timing::lock(&db) {
            Ok(g) => g,
            Err(_) => return,
        };
        db_guard
            .list_all_queued_submissions()
            .unwrap_or_default()
            .into_iter()
            .filter(|s| s.tin == profile.tin.full())
            .collect::<Vec<_>>()
    };

    for summary in summaries
        .into_iter()
        .filter(|s| s.status == FilingStatus::Queued)
        .filter(|s| crate::forms::can_queue_for_submission(&s.form_code))
    {
        let job_key = submission_job_key(
            &summary.form_code,
            &profile.tin.full(),
            summary.taxable_year,
            summary.month,
            summary.quarter,
        );

        let active_jobs = get_active_jobs();
        {
            let mut jobs = active_jobs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !jobs.insert(job_key.clone()) {
                continue; // Already running
            }
        }

        let profile_clone = profile.clone();
        let db_clone = db.clone();
        let cleanup = JobCleanup {
            key: job_key,
            active_jobs: active_jobs.clone(),
        };

        spawn_job(cleanup, async move {
            if summary.form_code == "2551Q" {
                process_queued_2551q(&summary, &profile_clone, db_clone.clone()).await;
                return;
            }
            if summary.form_code == "1601C" {
                process_queued_1601c(&summary, &profile_clone, db_clone.clone()).await;
            } else {
                process_queued_by_kind(&summary, &profile_clone, db_clone.clone()).await;
            }

            crate::ipc::post_db_changed();
        });
    }
}

/// `09/26` for a monthly return — the shape the maintainer reads at a glance.
pub fn monthly_period_label(taxable_year: u16, month: u8) -> String {
    format!("{month:02}/{:02}", taxable_year % 100)
}

/// `Q3/26` for a quarterly return.
pub fn quarterly_period_label(taxable_year: u16, quarter: u8) -> String {
    format!("Q{quarter}/{:02}", taxable_year % 100)
}

/// The app's one sentence for a send that reached nobody (a dry run, a
/// practice account, a Buwiz test TIN): the OS notice and the screens'
/// `test_sent` line (bir-app-api `send_state::TEST_SENT_LINE`) say it.
pub const TEST_SENT_LINE: &str = "Test send. Nothing reached the BIR.";

/// The first words of a test send's notice title.
const TEST_SEND_TITLE: &str = "Test send";

/// The generic desktop banner for a successful PUT. It deliberately knows
/// nothing about the taxpayer, TIN, form, period or mailbox: those details are
/// only in the in-app Notifications page. The arguments stay in the signature
/// of this long-standing helper so every caller is forced to reconsider which
/// text it needs; production callers use [`submission_alert_text`] for the
/// detailed page and `notification::notify` for the OS banner.
pub fn submission_notice<Tz: chrono::TimeZone>(
    profile: &TaxpayerProfile,
    form_code: &str,
    period_label: &str,
    sent_at: &chrono::DateTime<Tz>,
    dry_run: bool,
) -> (String, String)
where
    Tz::Offset: std::fmt::Display,
{
    let _ = (profile, form_code, period_label, sent_at);
    let (title, body) =
        crate::notification::banner_text(crate::notification::NotificationKind::ReturnSent {
            dry_run,
        });
    (title.to_string(), body.to_string())
}

/// The detailed submission text kept on the in-app Notifications page: who,
/// which return, when — not the IAF filename, which says none of that at a
/// glance. Returns `(title, body)`. A `dry_run` send reached nobody: its title
/// leads with "Test send" and its body's first line is [`TEST_SENT_LINE`]. It
/// never says the return was submitted, sent or filed.
pub(crate) fn submission_alert_text<Tz: chrono::TimeZone>(
    profile: &TaxpayerProfile,
    form_code: &str,
    period_label: &str,
    sent_at: &chrono::DateTime<Tz>,
    dry_run: bool,
) -> (String, String)
where
    Tz::Offset: std::fmt::Display,
{
    let at = sent_at.format("%I:%M %p, %-d %b %Y");
    let tin = profile.tin.formatted();
    if dry_run {
        return (
            format!(
                "{TEST_SEND_TITLE}: {form_code} {period_label} \u{2014} {}",
                profile.full_name
            ),
            format!("{TEST_SENT_LINE}\nTIN {tin}\nTested {at}"),
        );
    }
    (
        format!(
            "{form_code} {period_label} submitted \u{2014} {}",
            profile.full_name
        ),
        format!(
            "TIN {tin}\nSent {at}\nFiled under {}",
            profile.inbox_email()
        ),
    )
}

/// The generic desktop banner for a BIR receipt confirmation. It carries no
/// taxpayer detail; [`confirmation_alert_text`] is the detailed text kept on
/// the in-app Notifications page.
pub fn confirmation_notice(
    taxpayer_name: &str,
    tin_formatted: &str,
    form_code: &str,
    period_label: &str,
    bir_filename: &str,
    received_at: &str,
    mailbox: &str,
) -> (String, String) {
    let _ = (
        taxpayer_name,
        tin_formatted,
        form_code,
        period_label,
        bir_filename,
        received_at,
        mailbox,
    );
    let (title, body) = crate::notification::banner_text(
        crate::notification::NotificationKind::ConfirmationArrived,
    );
    (title.to_string(), body.to_string())
}

/// The detailed confirmation text kept on the in-app Notifications page. The
/// date and time are **BIR's** (from the confirmation email), not when the
/// poll found the mail. `received_at` is that date and time as one string,
/// e.g. `12 September 2026, 11:43 AM`.
pub(crate) fn confirmation_alert_text(
    taxpayer_name: &str,
    tin_formatted: &str,
    form_code: &str,
    period_label: &str,
    bir_filename: &str,
    received_at: &str,
    mailbox: &str,
) -> (String, String) {
    let title = format!("BIR received {form_code} {period_label} \u{2014} {taxpayer_name}");
    let body = format!(
        "TIN {tin_formatted}\nFile {bir_filename}\nReceived by BIR {received_at}\nConfirmation in {mailbox}"
    );
    (title, body)
}

/// Same event on the Notifications page. One entry per return period.
pub(crate) fn record_confirmation_alert(
    db: &Database,
    tin: &str,
    form_code: &str,
    period_label: &str,
    title: &str,
    body: &str,
    taxpayer_name: &str,
) {
    let kind = format!("form_confirmed:{form_code}:{period_label}");
    let detail = format!("{taxpayer_name}\n{body}");
    let _ = db.record_alert(
        Some(tin),
        &kind,
        crate::db::AlertSeverity::Info,
        title,
        &detail,
        crate::db::AlertAction::None,
    );
}

/// BIR's received stamp the way the confirmation email prints it. The
/// receipt table stores it normalized (`2026-09-12` / `12:11:00`); a banner
/// should read `12 September 2026, 12:11 PM`. Unrecognized shapes pass through.
pub fn display_received_at(received_date: &str, received_time: &str) -> String {
    let date = chrono::NaiveDate::parse_from_str(received_date.trim(), "%Y-%m-%d")
        .map(|d| d.format("%-d %B %Y").to_string())
        .unwrap_or_else(|_| received_date.trim().to_string());
    let time = chrono::NaiveTime::parse_from_str(received_time.trim(), "%H:%M:%S")
        .or_else(|_| chrono::NaiveTime::parse_from_str(received_time.trim(), "%H:%M"))
        .map(|t| t.format("%I:%M %p").to_string())
        .unwrap_or_else(|_| received_time.trim().to_string());
    format!("{date}, {time}")
}

/// `1601Cv2018` → `1601C`: the form code without the BIR schema revision.
pub fn form_code_from_form_type(form_type: &str) -> &str {
    form_type.split('v').next().unwrap_or(form_type)
}

/// Same event on the in-app Notifications page, so it is still there after the
/// banner is gone. One entry per return period; a re-file of the same period
/// bumps that entry rather than adding a second.
fn record_submission_alert(
    db: &Database,
    profile: &TaxpayerProfile,
    form_code: &str,
    period_label: &str,
    filename: &str,
    title: &str,
    body: &str,
) {
    let kind = format!("form_submitted:{form_code}:{period_label}");
    let detail = format!("{}\n{body}\nFile {filename}", profile.full_name);
    let _ = db.record_alert(
        Some(&profile.tin.full()),
        &kind,
        crate::db::AlertSeverity::Info,
        title,
        &detail,
        crate::db::AlertAction::None,
    );
}

pub fn schedule_email_poll(
    profile: &TaxpayerProfile,
    form_code: &str,
    tin: &str,
    taxable_year: u16,
    month: Option<u8>,
    quarter: Option<u8>,
    db_guard: &Database,
) {
    if !profile.is_email_tracking_active() {
        return;
    }

    // Command identity stays `bir_poll_email {email}` so the worker and
    // "Run now" path keep matching on the mailbox, not the display title.
    let email = profile.tracking_mailbox().unwrap_or("").to_string();
    let job_name = FilingJobLabel {
        form_code,
        taxable_year,
        month,
        quarter,
        tin,
        email: known_job_email(Some(email.as_str())),
    }
    .confirmation_poll_name();

    let jobs = db_guard.list_jobs().unwrap_or_default();
    let exists = jobs
        .iter()
        .any(|job| job.name == job_name && job.status != "Archived" && job.status != "Done");

    if !exists {
        let new_job = crate::db::Job {
            id: None,
            name: job_name,
            job_type: "System".to_string(),
            cron_expr: Some("0 * * * * *".to_string()),
            command: Some(format!("bir_poll_email {email}")),
            status: "Queued".to_string(),
            retries: 0,
            last_run_at: None,
            next_run_at: None,
            created_at: Utc::now().to_rfc3339(),
            output_log: None,
        };
        let _ = db_guard.save_job(new_job);
    }
}

/// Queue the confirmation poll again for every return on `inbox` that still waits for the BIR's email. Polls are
/// scheduled only when a send finishes, so once the inbox's way in changes (an app password saved, tracking switched
/// on, a connection verified) nothing else would bring back a poll that ended, for example the one a Google grant this
/// build cannot renew archived. [`schedule_email_poll`] keeps one job per return and skips an inbox whose tracking is
/// off. Returns how many returns were looked at.
/// Reconnect: a new Google grant for `inbox` is saved for the profiles on it, and the returns still waiting for the
/// BIR's email are polled again through it (as after Verify).
pub fn persist_google_grant_and_resume_polls(
    db_guard: &Database,
    inbox: &str,
    access_token: &str,
    refresh_token: &str,
    source_tin: Option<&str>,
) -> Result<crate::db::InboxOAuthPersistResult, crate::db::DbError> {
    let saved =
        db_guard.persist_google_oauth_for_inbox(inbox, access_token, refresh_token, source_tin)?;
    reschedule_email_polls_for_inbox(db_guard, inbox);
    Ok(saved)
}

pub fn reschedule_email_polls_for_inbox(db_guard: &Database, inbox: &str) -> usize {
    let mut seen = 0;
    let profiles = db_guard.list_profiles().unwrap_or_default();
    for profile in profiles
        .iter()
        .filter(|profile| crate::profile::inbox_emails_match(profile.inbox_email(), inbox))
    {
        let tin = profile.tin.full();
        for draft in db_guard
            .returns_awaiting_bir_email(&tin)
            .unwrap_or_default()
        {
            seen += 1;
            schedule_email_poll(
                profile,
                form_code_from_form_type(&draft.form_code),
                &draft.tin,
                draft.taxable_year,
                draft.month,
                draft.quarter,
                db_guard,
            );
        }
    }
    seen
}

/// The words kept on a return whose send ended with an unknown outcome, before the transport's own reason.
pub const OUTCOME_UNKNOWN: &str = "The BIR may have received this return. Check for the BIR's confirmation email before sending it again.";

/// A 1601C whose send went: Submitted, with the relay's receipt, the notice and the alert, and the inbox watched for
/// the BIR's email unless nothing reached the BIR. `xml_payload` is what was sent (`None` keeps the stored one).
#[allow(clippy::too_many_arguments)]
fn finish_sent_1601c(
    mut draft: Form1601CDraft,
    profile: &TaxpayerProfile,
    db: &Arc<Mutex<Database>>,
    claim_token: &str,
    filename: String,
    xml_payload: Option<String>,
    session_source: crate::transport::SftpEndpointSource,
    report: crate::transport::StoreReport,
) {
    let skip_email_poll =
        session_source.is_dry_run() || session_source.is_relay() || report.simulated;
    if skip_email_poll {
        info!(
            "Cron: dry run: simulated submission of queued 1601C {} (nothing sent to BIR)",
            crate::logging::submission_filename_for_log(filename.as_str())
        );
    } else {
        info!(
            "Cron: Successfully submitted queued 1601C {}",
            crate::logging::submission_filename_for_log(filename.as_str())
        );
    }
    let period_label = monthly_period_label(draft.taxable_year, draft.month);
    let (notice_title, notice_body) = submission_alert_text(
        profile,
        "1601C",
        &period_label,
        &chrono::Local::now(),
        skip_email_poll,
    );
    crate::notification::notify(crate::notification::NotificationKind::ReturnSent {
        dry_run: skip_email_poll,
    });
    let submitted_file = filename.clone();
    // Keep the exact bytes that were encrypted for PUT, so an
    // amendment can later snapshot what BIR received.
    if let Some(xml_payload) = xml_payload {
        draft.submitted_payload = Some(xml_payload);
    }
    draft.transition_to_submitted(filename, session_source.is_dry_run() || report.simulated);
    draft.submitted_via_relay = session_source.is_relay();
    draft.relay_receipt = report.relay_receipt;
    if let Ok(db_guard) = lock_timing::lock(db) {
        match db_guard.finish_claimed_1601c_submission(&draft, claim_token) {
            Ok(_) => {
                record_submission_alert(
                    &db_guard,
                    profile,
                    "1601C",
                    &period_label,
                    &submitted_file,
                    &notice_title,
                    &notice_body,
                );
                if skip_email_poll {
                    info!("Cron: 1601C dry-run PUT succeeded; skipping IMAP poll (source=dry-run)");
                } else {
                    schedule_email_poll(
                        profile,
                        "1601C",
                        &draft.tin,
                        draft.taxable_year,
                        Some(draft.month),
                        None,
                        &db_guard,
                    );
                }
                crate::dev_mode::maybe_auto_confirm(&db_guard, session_source, &submitted_file);
            }
            Err(error) => warn!(
                "Cron: 1601C {} was transmitted but its submission claim could not be finalized: {}",
                draft.period_code(),
                error
            ),
        }
    }
}

/// A claimed 1601C whose noted relay send this worker did not see end: ask Buwiz and act on what it says.
async fn reconcile_1601c<T: SubmissionTransport>(
    draft: Form1601CDraft,
    pending: String,
    profile: &TaxpayerProfile,
    db: Arc<Mutex<Database>>,
    transport: &T,
) {
    let Some(claim_token) = draft.submission_claim_token.clone() else {
        return;
    };
    let (tin, year, month) = (draft.tin.clone(), draft.taxable_year, draft.month);
    let clear = |db: &Arc<Mutex<Database>>| {
        if let Ok(db_guard) = lock_timing::lock(db) {
            let _ =
                db_guard.note_claimed_1601c_relay_pending(&tin, year, month, &claim_token, None);
        }
    };
    match reconcile_step(
        transport.settle_pending(&pending).await,
        &pending,
        Utc::now(),
    ) {
        Reconcile::Keep => return,
        Reconcile::Clear => clear(&db),
        Reconcile::Finish(report) => {
            let filename = draft.default_submission_filename();
            let payload = sent_payload(
                &pending,
                [
                    draft.try_to_bir_xml_payload().ok(),
                    draft.submitted_payload.clone(),
                ],
            );
            finish_sent_1601c(
                draft,
                profile,
                &db,
                &claim_token,
                filename,
                payload,
                crate::transport::SftpEndpointSource::Buwiz,
                report,
            );
        }
        Reconcile::Release(error) => {
            release_unaccepted(
                "1601C",
                &draft.period_code(),
                &error,
                &db,
                |db_guard, note, how| {
                    db_guard.release_unaccepted_claimed_1601c_submission(
                        &tin,
                        year,
                        month,
                        &claim_token,
                        note,
                        how,
                    )
                },
            );
            if matches!(error.no_receipt(), NoReceipt::Unknown) {
                clear(&db);
            }
        }
    }
    crate::ipc::post_db_changed();
}

/// A started send ended without a receipt. What proves that nothing reached the BIR lets the claim go through
/// `settle` (refused: back to Draft with the reason; failed or held: still Queued, retried later). Anything else is
/// an unknown outcome: once PUT has started, an error does not prove that the BIR received no bytes, so the durable
/// claim stays, just like after a process crash (retrying could duplicate a return), and the reason is recorded for
/// the person to resolve.
fn release_unaccepted(
    form_code: &str,
    period: &str,
    error: &crate::transport::TransportError,
    db: &Arc<Mutex<Database>>,
    settle: impl FnOnce(&Database, String, NoReceipt) -> Result<bool, crate::db::DbError>,
) {
    let how = error.no_receipt();
    let error_text = error.to_string();
    let error_for_log = crate::logging::redact_external_text_for_log(&error_text);
    let note = match how {
        NoReceipt::Unknown => {
            warn!(
                category = error.category(),
                "Cron: {form_code} {period} PUT ended after the network claim; outcome is unknown and the return is not sent again"
            );
            format!("{OUTCOME_UNKNOWN} ({error_text})")
        }
        _ => {
            warn!(
                category = error.category(),
                "Cron: {form_code} {period} was not sent: {error_for_log}"
            );
            error_text
        }
    };
    if let Ok(db_guard) = lock_timing::lock(db) {
        match settle(&db_guard, note, how) {
            Ok(true) => {}
            Ok(false) => info!(
                "Cron: {form_code} {period} was canceled or superseded before the relay's answer was recorded"
            ),
            Err(db_error) => warn!(
                "Cron: could not record the relay's answer for {form_code} {period}: {db_error}"
            ),
        }
    }
    crate::ipc::post_db_changed();
}

fn fail_draft_2551q(
    draft: &mut crate::forms::form_2551q::Form2551QDraft,
    revision: &Queued2551QRevision,
    db: Arc<Mutex<Database>>,
    error_msg: String,
) {
    settle_draft_2551q(draft, revision, db, error_msg, NoReceipt::Failed);
}

/// A 2551Q send that ended before any claim: a failed try, or a hold that is not one.
fn settle_draft_2551q(
    draft: &mut crate::forms::form_2551q::Form2551QDraft,
    revision: &Queued2551QRevision,
    db: Arc<Mutex<Database>>,
    error_msg: String,
    how: NoReceipt,
) {
    warn!(
        "Cron: Submission failed for {}: {}",
        draft.period_code(),
        error_msg
    );
    draft.settle_without_receipt(error_msg, how);
    if matches!(draft.status, FilingStatus::Draft) {
        warn!(
            "Cron: Max attempts reached for {}. Giving up.",
            draft.period_code()
        );
    } else if let Some(next) = &draft.next_retry_at {
        info!("Cron: Next retry not before {next}");
    }

    if let Ok(db_guard) = lock_timing::lock(&db) {
        match db_guard.replace_unclaimed_queued_2551q_submission(
            draft,
            &revision.fingerprint,
            &revision.next_retry_at,
            revision.submission_attempts,
        ) {
            Ok(true) => {}
            Ok(false) => info!(
                "Cron: Form {} was canceled or superseded before retry state persisted",
                draft.period_code()
            ),
            Err(error) => warn!(
                "Cron: Failed to persist submission failure for {}: {}",
                draft.period_code(),
                error
            ),
        }
    }
}

fn fail_draft_1601c(
    draft: &mut Form1601CDraft,
    revision: &Queued1601CRevision,
    db: Arc<Mutex<Database>>,
    error_message: String,
) {
    settle_draft_1601c(draft, revision, db, error_message, NoReceipt::Failed);
}

/// A 1601C send that ended before any claim: a failed try, or a hold that is not one.
fn settle_draft_1601c(
    draft: &mut Form1601CDraft,
    revision: &Queued1601CRevision,
    db: Arc<Mutex<Database>>,
    error_message: String,
    how: NoReceipt,
) {
    warn!(
        "Cron: 1601C submission preparation failed for {}: {}",
        draft.period_code(),
        error_message
    );
    draft.settle_without_receipt(error_message, how);
    if let Ok(db_guard) = lock_timing::lock(&db) {
        match db_guard.replace_unclaimed_queued_1601c_submission(
            draft,
            &revision.fingerprint,
            &revision.next_retry_at,
            revision.submission_attempts,
        ) {
            Ok(true) => {}
            Ok(false) => info!(
                "Cron: 1601C {} was canceled or superseded before retry state persisted",
                draft.period_code()
            ),
            Err(error) => warn!(
                "Cron: Failed to persist 1601C submission preparation failure for {}: {}",
                draft.period_code(),
                error
            ),
        }
    }
}

/// Status given to a shell-command job the App Store build will not run.
pub const SHELL_JOB_UNAVAILABLE_STATUS: &str = "Unavailable";

/// Recorded in `output_log` of a shell-command job the App Store build will
/// not run.
pub const SHELL_JOB_UNAVAILABLE_LOG: &str =
    "Shell commands are not available in the App Store version";

/// Whether this build can run user shell-command cron jobs. False in the Mac
/// App Store build, where `platform::run_shell_command` does not exist.
pub const SHELL_JOBS_SUPPORTED: bool = !cfg!(feature = "mas_build");

/// A job whose command is run through the platform shell, as opposed to the
/// built-in `bir_poll_email` command handled in-process.
pub fn is_shell_command_job(command: Option<&str>) -> bool {
    command.is_some_and(|cmd| !cmd.trim().is_empty() && !cmd.starts_with("bir_poll_email "))
}

/// Mark a shell-command job unavailable without running or deleting it.
/// Returns `true` when the job changed and needs saving. The schedule is left
/// as it was, so a direct-download build that imports this database still
/// runs the job.
fn mark_shell_job_unavailable(job: &mut crate::db::Job) -> bool {
    if job.status == SHELL_JOB_UNAVAILABLE_STATUS
        && job.output_log.as_deref() == Some(SHELL_JOB_UNAVAILABLE_LOG)
    {
        return false;
    }
    job.status = SHELL_JOB_UNAVAILABLE_STATUS.to_string();
    job.output_log = Some(SHELL_JOB_UNAVAILABLE_LOG.to_string());
    true
}

#[cfg(all(test, feature = "mas_build"))]
async fn process_generic_jobs(db: Arc<Mutex<Database>>) {
    process_generic_jobs_until(db, &CronStop::new()).await;
}

async fn process_generic_jobs_until(db: Arc<Mutex<Database>>, stop: &CronStop) {
    let jobs = {
        let db_guard = match lock_timing::lock(&db) {
            Ok(g) => g,
            Err(_) => return,
        };
        db_guard.list_jobs().unwrap_or_default()
    };

    let now = Utc::now();

    for mut job in jobs {
        if job.status == "Archived" || job.status == "Done" {
            continue;
        }

        if !SHELL_JOBS_SUPPORTED && is_shell_command_job(job.command.as_deref()) {
            if mark_shell_job_unavailable(&mut job) {
                warn!(
                    "Cron: job '{}' runs a shell command, which this build does not support",
                    job.name
                );
                if let Ok(db_guard) = lock_timing::lock(&db) {
                    let _ = db_guard.save_job(job.clone());
                }
            }
            continue;
        }

        // Self-heal legacy bad cron strings
        if let Some(ref mut expr) = job.cron_expr
            && expr.trim() == "* * * * *"
        {
            *expr = "0 * * * * *".to_string();
            if job.status == "Failed" {
                job.status = "Queued".to_string();
            }
            if let Ok(db_guard) = lock_timing::lock(&db) {
                let _ = db_guard.save_job(job.clone());
            }
        }

        let should_run = if let Some(ref next_run_str) = job.next_run_at {
            if let Ok(next_time) = chrono::DateTime::parse_from_rfc3339(next_run_str) {
                now >= next_time.with_timezone(&Utc)
            } else {
                false
            }
        } else {
            // No next run defined.
            if job.status == "Queued" {
                if let Some(ref expr) = job.cron_expr {
                    if !expr.trim().is_empty() {
                        // Initialize next run time
                        if let Ok(schedule) = cron::Schedule::from_str(expr)
                            && let Some(next_run) = schedule.upcoming(Utc).next()
                        {
                            job.next_run_at = Some(next_run.to_rfc3339());
                            if let Ok(db_guard) = lock_timing::lock(&db) {
                                let _ = db_guard.save_job(job.clone());
                            }
                        }
                        false // Wait for scheduled time
                    } else {
                        true // One-off job, run now
                    }
                } else {
                    true // One-off job, run now
                }
            } else {
                false
            }
        };

        if !should_run {
            continue;
        }

        // Set status to Running
        job.status = "Running".to_string();
        if let Ok(db_guard) = lock_timing::lock(&db) {
            let _ = db_guard.save_job(job.clone());
        }

        let job_key = format!("job:{}", job.id.unwrap_or(0));
        let active_jobs = get_active_jobs();
        {
            let mut active = active_jobs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !active.insert(job_key.clone()) {
                continue; // Already running
            }
        }

        let db_clone = db.clone();
        let job_stop = stop.clone();
        let cleanup = JobCleanup {
            key: job_key,
            active_jobs: active_jobs.clone(),
        };

        spawn_job(cleanup, async move {
            info!("Cron: Executing job '{}'", job.name);
            let mut success = true;

            if let Some(ref cmd) = job.command
                && !cmd.trim().is_empty()
            {
                if cmd.starts_with("bir_poll_email ") {
                    let email = cmd.trim_start_matches("bir_poll_email ").trim();
                    let (poll_success, still_pending, err_msg) =
                        crate::email::fetch_and_process_emails_for_address_until(
                            email,
                            db_clone.clone(),
                            &|| job_stop.is_stopped(),
                        );
                    if poll_success {
                        // Stopped early (the worker is ending), the poll read only part of the inbox; what is still
                        // waiting for its email keeps the job queued (`still_pending` is read back from the database).
                        let log = if job_stop.is_stopped() {
                            "Email polling stopped early: the queue worker is ending.".to_string()
                        } else {
                            "Email polling completed successfully.".to_string()
                        };
                        info!("Cron: Email polling job '{}': {}", job.name, log);
                        success = true;
                        job.output_log = Some(log);
                        if !still_pending {
                            job.status = "Archived".to_string(); // Completed processing for this email
                        }
                    } else {
                        let log = err_msg
                            .unwrap_or_else(|| "Email polling failed (unknown error).".to_string());
                        warn!("Cron: Email polling job '{}' failed: {}", job.name, log);
                        success = false;
                        job.output_log = Some(log);
                        // A failure that leaves nothing pending (a Google grant this build cannot renew, an address
                        // with no profile) ends the job rather than failing the same way every interval.
                        if !still_pending {
                            job.status = "Archived".to_string();
                        }
                    }
                } else {
                    // Unreachable in the App Store build (such jobs are
                    // skipped above); kept so the runner is compiled out.
                    #[cfg(feature = "mas_build")]
                    {
                        job.output_log = Some(SHELL_JOB_UNAVAILABLE_LOG.to_string());
                        success = false;
                    }
                    #[cfg(not(feature = "mas_build"))]
                    match crate::platform::run_shell_command(cmd).await {
                        Ok(output) => {
                            let stdout_str = String::from_utf8_lossy(&output.stdout);
                            let stderr_str = String::from_utf8_lossy(&output.stderr);
                            let log = format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout_str, stderr_str);
                            job.output_log = Some(log);
                            if output.status.success() {
                                info!("Cron: Job '{}' completed successfully.", job.name);
                            } else {
                                warn!(
                                    "Cron: Job '{}' failed with code: {:?} stderr: {}",
                                    job.name, output.status, stderr_str
                                );
                                success = false;
                            }
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to start job: {}", e);
                            warn!("Cron: Job '{}' failed to start: {}", job.name, e);
                            job.output_log = Some(err_msg);
                            success = false;
                        }
                    }
                }
            }

            job.last_run_at = Some(now.to_rfc3339());

            if let Some(ref cron_expr) = job.cron_expr {
                if !cron_expr.trim().is_empty() {
                    if let Ok(schedule) = cron::Schedule::from_str(cron_expr) {
                        if let Some(next_run) = schedule.upcoming(Utc).next() {
                            job.next_run_at = Some(next_run.to_rfc3339());
                            if job.status != "Archived" && job.status != "Done" {
                                job.status = "Queued".to_string();
                            }
                            job.retries = if success { 0 } else { job.retries + 1 };
                        } else {
                            job.status = "Done".to_string(); // cron will never run again
                        }
                    } else {
                        warn!("Cron: Invalid cron expression for job '{}'", job.name);
                        job.status = "Failed".to_string();
                    }
                    if let Ok(db_guard) = lock_timing::lock(&db_clone) {
                        let _ = db_guard.save_job(job.clone());
                    }
                } else {
                    // Treated as one-off if empty string
                    if success {
                        if let Ok(db_guard) = lock_timing::lock(&db_clone) {
                            let _ = db_guard.delete_job(job.id.unwrap());
                        }
                    } else {
                        job.retries += 1;
                        job.status = "Failed".to_string();
                        if let Ok(db_guard) = lock_timing::lock(&db_clone) {
                            let _ = db_guard.save_job(job.clone());
                        }
                    }
                }
            } else {
                // One-off job
                if success {
                    if let Ok(db_guard) = lock_timing::lock(&db_clone) {
                        let _ = db_guard.delete_job(job.id.unwrap());
                    }
                } else {
                    job.retries += 1;
                    job.status = "Failed".to_string();
                    if let Ok(db_guard) = lock_timing::lock(&db_clone) {
                        let _ = db_guard.save_job(job.clone());
                    }
                }
            }

            crate::ipc::post_db_changed();
        });
    }
}

/// Put back to Queued every job saved as Running: none can be running as a worker starts (a database has one worker
/// at a time, and the one before ended with its tasks), so such a job was cut off, by an engine's end that dropped its
/// task or by the process ending. A one-off job left Running would otherwise never run again.
///
/// So a job runs at least once, not exactly once: one cut off after its work and before its end was saved (a one-off's
/// delete, a scheduled job's next run) runs that work again. Jobs never send returns: they poll an inbox (a second poll
/// reads the same mail) or run a command a person scheduled, which can then run twice.
fn requeue_jobs_left_running(db: &Arc<Mutex<Database>>) {
    let Ok(db) = lock_timing::lock(db) else {
        return;
    };
    for mut job in db.list_jobs().unwrap_or_default() {
        if job.status == "Running" {
            info!(
                "Cron: job '{}' was cut off while running; queued again",
                job.name
            );
            job.status = "Queued".to_string();
            let _ = db.save_job(job);
        }
    }
}

/// Spawn one job on the worker's runtime with its key's `cleanup` inside the task's future: the key leaves
/// [`get_active_jobs`] however the task ends, finished, panicked, or dropped by a runtime shut down mid-send (an
/// engine that ended) or before it was ever polled. The next worker then reads the return's own state, never a key a
/// dead task left behind.
fn spawn_job(cleanup: JobCleanup, job: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(async move {
        let _cleanup = cleanup;
        job.await;
    });
}

struct JobCleanup {
    key: String,
    active_jobs: Arc<Mutex<HashSet<String>>>,
}

impl Drop for JobCleanup {
    fn drop(&mut self) {
        self.active_jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.key);
    }
}

#[cfg(test)]
mod tests {
    fn shell_job(command: &str) -> crate::db::Job {
        crate::db::Job {
            id: None,
            name: "Nightly script".into(),
            job_type: "Custom".into(),
            cron_expr: Some("0 * * * * *".into()),
            command: Some(command.into()),
            status: "Queued".into(),
            retries: 0,
            last_run_at: None,
            next_run_at: Some("2000-01-01T00:00:00+00:00".into()),
            created_at: String::new(),
            output_log: None,
        }
    }

    /// A job saved as Running whose task went with an engine's end (or the process) is queued again when the next
    /// worker starts: a one-off job (no schedule) left Running would never run again. Other jobs are left as they are.
    #[test]
    fn a_job_cut_off_while_running_is_queued_again_as_a_worker_starts() {
        use std::sync::{Arc, Mutex};
        let db = crate::db::Database::open_in_memory_for_tests().expect("db");
        let mut cut_off = shell_job("bir_poll_email juana@example.com");
        cut_off.cron_expr = None;
        cut_off.next_run_at = None;
        cut_off.status = "Running".into();
        let cut_off = db.save_job(cut_off).expect("save").id.expect("an id");
        let mut failed = shell_job("echo hi");
        failed.status = "Failed".into();
        let failed = db.save_job(failed).expect("save").id.expect("an id");
        let db = Arc::new(Mutex::new(db));

        super::requeue_jobs_left_running(&db);

        let jobs = db.lock().unwrap().list_jobs().unwrap();
        let status = |id| {
            jobs.iter()
                .find(|job| job.id == Some(id))
                .unwrap()
                .status
                .clone()
        };
        assert_eq!(status(cut_off), "Queued");
        assert_eq!(status(failed), "Failed");
    }

    /// The same through a real start: the worker's loop (`run_cron_jobs`, as an engine's launch starts it) queues
    /// again a job saved as Running before its first tick. The job is a yearly one not due for centuries, so nothing
    /// runs it after: what is read is the requeue alone. In a child process of its own: the loop's wake channel and
    /// shutdown generation are the process's.
    #[test]
    fn a_real_worker_start_queues_again_a_job_cut_off_while_running() {
        const CHILD: &str = "BIR_CORE_CRON_REQUEUE_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "background_cron::tests::a_real_worker_start_queues_again_a_job_cut_off_while_running",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env("EBIR_TEST_ENV", "1")
                .env("BIR_SFTP_DRY_RUN", "1")
                .env_remove("BIR_SFTP_LIVE")
                .env_remove("BIR_RELAY_URL")
                .status()
                .unwrap();
            assert!(status.success(), "the child process failed");
            return;
        }
        use std::sync::{Arc, Mutex};
        let db = crate::db::Database::open_in_memory_for_tests().expect("db");
        let mut cut_off = shell_job("bir_poll_email nobody@example.test");
        cut_off.cron_expr = Some("0 0 0 1 1 *".into());
        cut_off.next_run_at = Some("2999-01-01T00:00:00+00:00".into());
        cut_off.status = "Running".into();
        let cut_off = db.save_job(cut_off).expect("save").id.expect("an id");
        let db = Arc::new(Mutex::new(db));
        let status = |db: &Arc<Mutex<crate::db::Database>>| {
            db.lock()
                .unwrap()
                .list_jobs()
                .unwrap()
                .into_iter()
                .find(|job| job.id == Some(cut_off))
                .map(|job| job.status)
        };

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let stop = CronStop::new();
            let worker = tokio::spawn(run_cron_jobs(db.clone(), stop.clone()));
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
            while status(&db).as_deref() == Some("Running") {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "the worker never queued the job again"
                );
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            // A tick has had the time to run anything due; this job is not due.
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            assert_eq!(
                status(&db).as_deref(),
                Some("Queued"),
                "queued again, and not run"
            );
            stop.stop();
            tokio::time::timeout(std::time::Duration::from_secs(5), worker)
                .await
                .expect("the loop stopped on its signal")
                .unwrap();
        });
    }

    #[test]
    fn only_non_builtin_commands_are_shell_jobs() {
        assert!(super::is_shell_command_job(Some("echo hi")));
        assert!(!super::is_shell_command_job(Some("bir_poll_email a@b.c")));
        assert!(!super::is_shell_command_job(Some("   ")));
        assert!(!super::is_shell_command_job(None));
    }

    #[test]
    fn marking_unavailable_keeps_the_schedule_and_is_idempotent() {
        let mut job = shell_job("echo hi");
        assert!(super::mark_shell_job_unavailable(&mut job));
        assert_eq!(job.status, super::SHELL_JOB_UNAVAILABLE_STATUS);
        assert_eq!(
            job.output_log.as_deref(),
            Some(super::SHELL_JOB_UNAVAILABLE_LOG)
        );
        assert_eq!(job.cron_expr.as_deref(), Some("0 * * * * *"));
        assert!(job.next_run_at.is_some());
        assert!(!super::mark_shell_job_unavailable(&mut job));
    }

    #[test]
    fn shell_jobs_are_supported_exactly_when_not_an_app_store_build() {
        assert_eq!(super::SHELL_JOBS_SUPPORTED, !cfg!(feature = "mas_build"));
    }

    /// The App Store build must neither run nor delete a due shell job: the
    /// row stays, marked with the reason.
    #[cfg(feature = "mas_build")]
    #[tokio::test]
    async fn app_store_build_marks_due_shell_job_without_running_it() {
        use std::sync::{Arc, Mutex};
        let db = crate::db::Database::open_in_memory_for_tests().expect("db");
        let marker = tempfile::tempdir().expect("tempdir").path().join("ran");
        let saved = db
            .save_job(shell_job(&format!("touch {}", marker.display())))
            .expect("save");
        let db = Arc::new(Mutex::new(db));

        super::process_generic_jobs(db.clone()).await;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let jobs = db.lock().unwrap().list_jobs().expect("list");
        let job = jobs
            .into_iter()
            .find(|j| j.id == saved.id)
            .expect("job row must not be deleted");
        assert_eq!(job.status, super::SHELL_JOB_UNAVAILABLE_STATUS);
        assert_eq!(
            job.output_log.as_deref(),
            Some(super::SHELL_JOB_UNAVAILABLE_LOG)
        );
        assert!(!marker.exists(), "the shell command must not run");
    }

    #[test]
    fn period_labels_read_as_month_or_quarter_over_two_digit_year() {
        assert_eq!(monthly_period_label(2026, 9), "09/26");
        assert_eq!(monthly_period_label(2026, 12), "12/26");
        assert_eq!(quarterly_period_label(2026, 3), "Q3/26");
    }

    #[test]
    fn submission_alert_text_keeps_taxpayer_tin_form_period_and_time() {
        let profile = crate::filing_queue::mandatory_lab_1601c_profile();
        let sent_at = chrono::Local
            .with_ymd_and_hms(2026, 9, 12, 15, 17, 25)
            .unwrap();
        let (title, body) = submission_alert_text(&profile, "1601C", "09/26", &sent_at, false);
        assert_eq!(title, "1601C 09/26 submitted \u{2014} Juan Dela Cruz");
        assert_eq!(
            body,
            "TIN 000-000-000-00000\nSent 03:17 PM, 12 Sep 2026\nFiled under codeitlikemiley@gmail.com"
        );
    }

    #[test]
    fn submission_alert_text_marks_dry_run_without_claiming_filing() {
        let profile = crate::filing_queue::mandatory_lab_1601c_profile();
        let sent_at = chrono::Local
            .with_ymd_and_hms(2026, 9, 12, 15, 17, 25)
            .unwrap();
        let (title, body) = submission_alert_text(&profile, "1601C", "09/26", &sent_at, true);
        assert_eq!(title, "Test send: 1601C 09/26 \u{2014} Juan Dela Cruz");
        assert_eq!(
            body,
            "Test send. Nothing reached the BIR.\nTIN 000-000-000-00000\nTested 03:17 PM, 12 Sep 2026"
        );
        assert_never_filed_alert(&title, &body);
    }

    #[test]
    fn confirmation_alert_text_keeps_the_detailed_in_app_text() {
        let (title, body) = confirmation_alert_text(
            "Juan Dela Cruz",
            "000-000-000-00000",
            "1601C",
            "10/26",
            "00000000000000-1601Cv2018-102026.xml",
            "12 September 2026, 11:43 AM",
            "codeitlikemiley@gmail.com",
        );
        assert_eq!(title, "BIR received 1601C 10/26 \u{2014} Juan Dela Cruz");
        assert_eq!(
            body,
            "TIN 000-000-000-00000\nFile 00000000000000-1601Cv2018-102026.xml\nReceived by BIR 12 September 2026, 11:43 AM\nConfirmation in codeitlikemiley@gmail.com"
        );
    }

    #[test]
    fn submission_notice_marks_dry_run_in_title_and_body() {
        let profile = crate::filing_queue::mandatory_lab_1601c_profile();
        let sent_at = chrono::FixedOffset::east_opt(8 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 9, 12, 15, 17, 25)
            .unwrap();
        let (title, body) = submission_notice(&profile, "1601C", "09/26", &sent_at, true);
        assert_eq!(title, "eBIRForms");
        assert_eq!(
            body,
            "Test send. Nothing reached the BIR. Open eBIRForms to see it."
        );
        assert_no_identity_in_notice(&title, &body);
    }

    /// A phone banner shows the title's first few words and cuts the rest: a test send's title leads with "Test
    /// send" whatever the registered name's length, and the body's first line is the app's sentence for it.
    #[test]
    fn a_test_send_notice_leads_with_test_send_before_a_long_name() {
        let mut profile = crate::filing_queue::mandatory_lab_1601c_profile();
        profile.full_name =
            "Juan Dela Cruz Trading and General Merchandise Corporation of the Philippines".into();
        let sent_at = chrono::FixedOffset::east_opt(8 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 9, 12, 15, 17, 25)
            .unwrap();
        let (title, body) = submission_notice(&profile, "1601C", "09/26", &sent_at, true);
        assert_eq!(title, "eBIRForms");
        assert_eq!(
            body,
            "Test send. Nothing reached the BIR. Open eBIRForms to see it."
        );
        assert!(!body.contains(&profile.full_name), "{body}");
        assert_no_identity_in_notice(&title, &body);
    }

    /// The OS banner is a fixed sentence: neither the taxpayer's name nor any
    /// spelling of the TIN can be in it. The Notifications page keeps the
    /// detailed alert; the banner only says where to look.
    fn assert_no_identity_in_notice(title: &str, body: &str) {
        for text in [title, body] {
            assert!(!text.contains("Juan Dela Cruz"), "{text}");
            for tin in ["00000000000000", "000-000-000-00000", "000-000-000"] {
                assert!(!text.contains(tin), "{tin} in {text}");
            }
        }
    }

    /// The stored test-send alert still leads with its mark and never says
    /// the return was submitted, sent or filed; only the OS banner is generic.
    fn assert_never_filed_alert(title: &str, body: &str) {
        assert!(title.starts_with("Test send: "), "{title}");
        assert!(body.lines().any(|line| line == TEST_SENT_LINE), "{body}");
        for text in [title, body] {
            let lower = text.to_lowercase();
            for word in ["submitted", "sent ", "filed"] {
                assert!(!lower.contains(word), "{word:?} in {text:?}");
            }
        }
    }

    #[test]
    fn submission_notice_names_taxpayer_tin_form_period_and_time() {
        let profile = crate::filing_queue::mandatory_lab_1601c_profile();
        let sent_at = chrono::FixedOffset::east_opt(8 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 9, 12, 15, 17, 25)
            .unwrap();
        let (title, body) = submission_notice(&profile, "1601C", "09/26", &sent_at, false);
        assert_eq!(title, "eBIRForms");
        assert_eq!(body, "A return was sent. Open eBIRForms to see it.");
        assert_no_identity_in_notice(&title, &body);
    }

    #[test]
    fn confirmation_notice_carries_bir_time_file_and_mailbox() {
        let (title, body) = confirmation_notice(
            "Juan Dela Cruz",
            "000-000-000-00000",
            "1601C",
            "10/26",
            "00000000000000-1601Cv2018-102026.xml",
            "12 September 2026, 11:43 AM",
            "codeitlikemiley@gmail.com",
        );
        assert_eq!(title, "eBIRForms");
        assert_eq!(
            body,
            "BIR's confirmation arrived. Open eBIRForms to see it."
        );
        assert_no_identity_in_notice(&title, &body);
        assert_eq!(
            display_received_at("2026-09-12", "12:11:00"),
            "12 September 2026, 12:11 PM"
        );
        assert_eq!(
            display_received_at("2026-09-12", "11:43"),
            "12 September 2026, 11:43 AM"
        );
        assert_eq!(
            display_received_at("12 September 2026", "11:43 AM"),
            "12 September 2026, 11:43 AM"
        );
        assert_eq!(form_code_from_form_type("1601Cv2018"), "1601C");
        assert_eq!(form_code_from_form_type("2551Qv2018"), "2551Q");
    }

    #[test]
    fn submission_alert_lands_on_the_notifications_page_once_per_period() {
        let db = Database::open_in_memory_for_tests().expect("db");
        let profile = crate::filing_queue::mandatory_lab_1601c_profile();
        let profile = db.save_profile(profile).expect("profile");
        for _ in 0..2 {
            record_submission_alert(
                &db,
                &profile,
                "1601C",
                "09/26",
                "00000000000000-1601Cv2018-092026#x#.xml",
                "1601C 09/26 submitted \u{2014} Juan Dela Cruz",
                "TIN 000-000-000-00000\nSent 03:17 PM, 12 Sep 2026",
            );
        }
        let alerts = db.list_active_alerts(None).expect("alerts");
        let ours: Vec<_> = alerts
            .iter()
            .filter(|alert| alert.kind == "form_submitted:1601C:09/26")
            .collect();
        assert_eq!(ours.len(), 1, "{alerts:?}");
        assert_eq!(ours[0].severity, crate::db::AlertSeverity::Info);
        assert_eq!(ours[0].action, crate::db::AlertAction::None);
        assert!(
            ours[0].detail.contains("Juan Dela Cruz"),
            "{}",
            ours[0].detail
        );
        assert!(ours[0].detail.contains("TIN 000-000-000-00000"));
        assert!(ours[0].detail.contains("092026"));
    }

    use super::*;
    use crate::db::{Database, Job};
    use crate::forms::form_1601c::Form1601CDraft;
    use crate::forms::form_2551q::Item13Election;
    use crate::profile::{EoptTier, IncomeTaxElection, TaxElectionHistory, TaxpayerProfile};
    use chrono::{Datelike, TimeZone};
    use tempfile::NamedTempFile;

    fn is_email_poll_job(job: &Job) -> bool {
        job.command
            .as_deref()
            .is_some_and(|command| command.starts_with("bir_poll_email "))
            || job.name.contains(" confirmation for ")
            || job.name.contains("confirmation email")
            || job.name.starts_with("Poll Receipts:")
    }

    #[test]
    fn retry_gate_is_due_only_for_absent_or_elapsed_valid_timestamps() {
        let now = Utc
            .with_ymd_and_hms(2026, 7, 24, 12, 0, 0)
            .single()
            .unwrap();

        assert!(queued_retry_is_due(None, now).unwrap());
        assert!(queued_retry_is_due(Some("2026-07-24T11:59:59Z"), now).unwrap());
        assert!(!queued_retry_is_due(Some("2026-07-24T12:00:01Z"), now).unwrap());
    }

    #[test]
    fn retry_gate_rejects_malformed_non_null_metadata() {
        let now = Utc
            .with_ymd_and_hms(2026, 7, 24, 12, 0, 0)
            .single()
            .unwrap();

        assert!(queued_retry_is_due(Some("not-rfc3339"), now).is_err());
        assert!(queued_retry_is_due(Some(""), now).is_err());
    }

    fn test_profile() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Queue Guard Taxpayer",
            "tin": {
                "segment1": "123",
                "segment2": "456",
                "segment3": "789",
                "branch": "000"
            },
            "rdo_code": "018",
            "line_of_business": "Retail",
            "registered_address": "Manila",
            "zip_code": "1000",
            "phone": "09123456789",
            "email": "guard@example.com",
            "default_form_type": "2551Qv2018",
            "taxpayer_type": "Individual",
            "business_start_date": "2010-01-01"
        }))
        .unwrap()
    }

    fn reviewed_profile() -> TaxpayerProfile {
        let mut profile = test_profile();
        profile.tax_elections.push(TaxElectionHistory {
            taxable_year: 2024,
            election: IncomeTaxElection::GraduatedUnspecified,
            elected_at: chrono::NaiveDateTime::default(),
            source_form: "2551Qv2018".to_string(),
        });
        profile.ensure_profile_version_ledger();
        profile
    }

    fn queued_draft(profile: &TaxpayerProfile) -> Form2551QDraft {
        let mut draft = Form2551QDraft::new_from_effective_profile(profile, 2024, 1);
        draft.item_13_election = Item13Election::Graduated;
        draft
            .transition_to_queued()
            .expect("the reviewed draft should queue");
        draft
    }

    fn queued_1601c_draft(profile: &TaxpayerProfile) -> Form1601CDraft {
        let mut draft = Form1601CDraft::new_from_profile(profile, 2024, 5);
        draft.any_taxes_withheld = false;
        draft
            .transition_to_queued()
            .expect("the reviewed 1601C draft should queue");
        draft
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RecordedSubmission {
        form_type: String,
        filename: String,
        payload: Vec<u8>,
    }

    #[derive(Debug, Clone, Copy)]
    enum TestTransportOutcome {
        Success,
        PreStore,
        UnknownIo,
        /// The relay answered 4xx after the claim: it kept nothing.
        RelayRefused,
        /// The relay answered 503 (or was not there) after the claim: it kept nothing.
        RelayUnavailable,
        /// The Buwiz relay gave a receipt for a real send.
        BuwizSent,
        /// The Buwiz relay gave a receipt, and the server only simulated the send.
        BuwizSimulated,
        /// The Buwiz relay's `result` for a test TIN ([`crate::relay::DRY_RUN_RESULT_FIXTURE`]), as the client hands
        /// it over.
        BuwizDryRunResult,
        /// The Buwiz relay refused the return itself (a TIN refusal).
        BuwizRefused,
        /// The Buwiz relay tried and the BIR's way in did not take it.
        BuwizFailed,
        /// Buwiz held the send after the claim: not a try.
        BuwizHeld,
        /// Buwiz was not ready before any claim (signed out, unreachable): not a try.
        BuwizHeldBeforeClaim,
        /// The Buwiz relay cannot say whether the BIR has the return.
        BuwizUnknown,
        /// The app is stopped during the send, after both notes: the send never ends.
        BuwizKilled,
    }

    /// The refusal the Buwiz test outcomes carry.
    const BUWIZ_REFUSAL: &str = "This TIN is not on your Buwiz account. (tin_not_active)";
    /// The hold the Buwiz test outcomes carry.
    const BUWIZ_HOLD: &str = "BIR is not accepting returns right now. (bir_offline)";
    /// The receipt the Buwiz test outcomes carry (its form is not checked here).
    const BUWIZ_RECEIPT: &str = "header.payload.signature";

    fn buwiz_hold_until() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2099, 1, 1, 0, 0, 0).unwrap()
    }

    struct RecordingSubmissionTransport {
        /// The plain return handed over beside each encrypted file.
        plain: Mutex<Vec<Vec<u8>>>,
        calls: Mutex<Vec<RecordedSubmission>>,
        outcome: TestTransportOutcome,
        source: crate::transport::SftpEndpointSource,
        /// What Buwiz says of a noted send ([`SubmissionTransport::settle_pending`]); not yet by default.
        settles: fn(&bir_submit::PendingSend) -> crate::relay::Settlement,
        /// Each noted send it was asked about.
        asked: Mutex<Vec<bir_submit::PendingSend>>,
        /// Each note the send made, as the worker kept it (`Err`: the worker refused it).
        noted: Mutex<Vec<Result<(), String>>>,
    }

    impl RecordingSubmissionTransport {
        fn new(outcome: TestTransportOutcome) -> Self {
            Self {
                plain: Mutex::new(Vec::new()),
                calls: Mutex::new(Vec::new()),
                outcome,
                source: crate::transport::SftpEndpointSource::Env,
                settles: |_| crate::relay::Settlement::NotYet,
                asked: Mutex::new(Vec::new()),
                noted: Mutex::new(Vec::new()),
            }
        }

        fn relay(outcome: TestTransportOutcome) -> Self {
            Self {
                plain: Mutex::new(Vec::new()),
                calls: Mutex::new(Vec::new()),
                outcome,
                source: crate::transport::SftpEndpointSource::Relay,
                settles: |_| crate::relay::Settlement::NotYet,
                asked: Mutex::new(Vec::new()),
                noted: Mutex::new(Vec::new()),
            }
        }

        fn dry_run(outcome: TestTransportOutcome) -> Self {
            Self {
                plain: Mutex::new(Vec::new()),
                calls: Mutex::new(Vec::new()),
                outcome,
                source: crate::transport::SftpEndpointSource::DryRun,
                settles: |_| crate::relay::Settlement::NotYet,
                asked: Mutex::new(Vec::new()),
                noted: Mutex::new(Vec::new()),
            }
        }

        fn buwiz(outcome: TestTransportOutcome) -> Self {
            Self {
                plain: Mutex::new(Vec::new()),
                calls: Mutex::new(Vec::new()),
                outcome,
                source: crate::transport::SftpEndpointSource::Buwiz,
                settles: |_| crate::relay::Settlement::NotYet,
                asked: Mutex::new(Vec::new()),
                noted: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<RecordedSubmission> {
            self.calls
                .lock()
                .expect("recorded transport calls should not be poisoned")
                .clone()
        }
    }

    impl SubmissionTransport for RecordingSubmissionTransport {
        type Session = String;

        fn session_source(&self, _session: &Self::Session) -> crate::transport::SftpEndpointSource {
            self.source
        }

        fn open_session<'a>(
            &'a self,
            form_type: &'a str,
            _tin: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Self::Session, crate::transport::TransportError>>
                    + Send
                    + 'a,
            >,
        > {
            let outcome = self.outcome;
            let form_type = form_type.to_string();
            Box::pin(async move {
                match outcome {
                    TestTransportOutcome::PreStore => {
                        Err(crate::transport::TransportError::Io(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "simulated sftp connect timeout",
                        )))
                    }
                    TestTransportOutcome::BuwizHeldBeforeClaim => {
                        Err(crate::transport::TransportError::RelayHeld {
                            reason: "Sign in to send returns.".into(),
                            wait_until: None,
                        })
                    }
                    _ => Ok(form_type),
                }
            })
        }

        fn store_session<'a>(
            &'a self,
            session: Self::Session,
            filename: &'a str,
            payload: &'a [u8],
            xml: &'a [u8],
            note: bir_submit::Note<'a>,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            crate::transport::StoreReport,
                            crate::transport::TransportError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            // Only a send through the Buwiz relay notes its progress, as the client does: once the server made
            // the submission, then before the first connection toward the BIR.
            if self.source == crate::transport::SftpEndpointSource::Buwiz {
                let pending = test_pending(xml, false, Utc::now());
                let mut noted = self.noted.lock().unwrap();
                noted.push(note(&pending));
                noted.push(note(&bir_submit::PendingSend {
                    opening: true,
                    ..pending
                }));
            }
            self.plain
                .lock()
                .expect("recorded plain returns should not be poisoned")
                .push(xml.to_vec());
            self.calls
                .lock()
                .expect("recorded transport calls should not be poisoned")
                .push(RecordedSubmission {
                    form_type: session,
                    filename: filename.to_string(),
                    payload: payload.to_vec(),
                });
            let outcome = self.outcome;
            Box::pin(async move {
                match outcome {
                    TestTransportOutcome::Success => Ok(crate::transport::StoreReport::default()),
                    TestTransportOutcome::BuwizSent => Ok(crate::transport::StoreReport {
                        relay_receipt: Some(BUWIZ_RECEIPT.into()),
                        simulated: false,
                    }),
                    TestTransportOutcome::BuwizSimulated => Ok(crate::transport::StoreReport {
                        relay_receipt: Some(BUWIZ_RECEIPT.into()),
                        simulated: true,
                    }),
                    TestTransportOutcome::BuwizDryRunResult => {
                        Ok(crate::relay::dry_run_result_fixture_report())
                    }
                    TestTransportOutcome::BuwizRefused => Err(
                        crate::transport::TransportError::Refused(BUWIZ_REFUSAL.into()),
                    ),
                    TestTransportOutcome::BuwizFailed => {
                        Err(crate::transport::TransportError::RelayFailed(
                            "BIR's upload server did not answer".into(),
                        ))
                    }
                    TestTransportOutcome::BuwizHeld => {
                        Err(crate::transport::TransportError::RelayHeld {
                            reason: BUWIZ_HOLD.into(),
                            wait_until: Some(buwiz_hold_until()),
                        })
                    }
                    TestTransportOutcome::BuwizUnknown => {
                        Err(crate::transport::TransportError::RelayUnknown(
                            "The relay ended without a result.".into(),
                        ))
                    }
                    TestTransportOutcome::BuwizHeldBeforeClaim => {
                        panic!("PUT must not run when Buwiz was not ready")
                    }
                    TestTransportOutcome::BuwizKilled => std::future::pending().await,
                    TestTransportOutcome::UnknownIo => {
                        Err(crate::transport::TransportError::Io(std::io::Error::new(
                            std::io::ErrorKind::ConnectionReset,
                            "simulated unknown network outcome",
                        )))
                    }
                    TestTransportOutcome::RelayRefused => Err(
                        crate::transport::TransportError::RelayRefused("422: bad filename".into()),
                    ),
                    TestTransportOutcome::RelayUnavailable => Err(
                        crate::transport::TransportError::RelayUnavailable("503: down".into()),
                    ),
                    TestTransportOutcome::PreStore => {
                        panic!("PUT must not run after a pre-store SFTP failure")
                    }
                }
            })
        }

        fn settle_pending<'a>(
            &'a self,
            pending: &'a str,
        ) -> Pin<Box<dyn Future<Output = crate::relay::Settlement> + Send + 'a>> {
            let pending: bir_submit::PendingSend =
                serde_json::from_str(pending).expect("the worker keeps the client's record");
            let settled = (self.settles)(&pending);
            self.asked.lock().unwrap().push(pending);
            Box::pin(async move { settled })
        }
    }

    impl RecordingSubmissionTransport {
        /// A Buwiz transport whose server says `settles` of a noted send.
        fn settling(
            outcome: TestTransportOutcome,
            settles: fn(&bir_submit::PendingSend) -> crate::relay::Settlement,
        ) -> Self {
            Self {
                settles,
                ..Self::buwiz(outcome)
            }
        }

        fn asked(&self) -> usize {
            self.asked.lock().unwrap().len()
        }
    }

    /// The noted send of the plain return `xml`.
    fn test_pending(
        xml: &[u8],
        opening: bool,
        noted_at: chrono::DateTime<Utc>,
    ) -> bir_submit::PendingSend {
        use sha2::Digest;
        bir_submit::PendingSend {
            server: "http://localhost:3008".into(),
            submission_id: "s1".into(),
            jti: "jti-1".into(),
            xml_sha256: hex::encode(sha2::Sha256::digest(xml)),
            opening,
            noted_at,
        }
    }

    /// The stored row's JSON exactly as written.
    fn stored_json(db: &Arc<Mutex<Database>>, form_code: &str, tin: &str) -> String {
        db.lock()
            .unwrap()
            .conn
            .query_row(
                "SELECT data_json FROM form_drafts WHERE form_code = ?1 AND tin = ?2",
                rusqlite::params![form_code, tin],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn queued_1601c_summary(id: i64, draft: &Form1601CDraft) -> FormDraftSummary {
        FormDraftSummary {
            id,
            tin: draft.tin.clone(),
            form_code: "1601C".to_string(),
            taxable_year: draft.taxable_year,
            quarter: None,
            month: Some(draft.month),
            status: FilingStatus::Queued,
            updated_at: draft.updated_at.clone(),
        }
    }

    fn assert_exact_1601c_transport_call(call: &RecordedSubmission, queued: &Form1601CDraft) {
        assert_eq!(call.form_type, "1601Cv2018");
        assert_eq!(call.filename, queued.default_submission_filename());
        let expected_xml = queued
            .try_to_bir_xml_payload()
            .expect("the reviewed queued draft should produce exact XML");
        assert_ne!(call.payload, expected_xml.as_bytes());
        let decrypted =
            crate::crypto::decrypt_and_decompress(&call.payload, crate::crypto::BIR_IAF_PASSPHRASE)
                .expect(
                    "the captured transport payload should use the BIR IAF encryption boundary",
                );
        let decrypted_xml = String::from_utf8(decrypted)
            .expect("the decrypted transport payload should contain UTF-8 XML");
        assert_eq!(
            crate::bir_xml::parse_bir_xml_with_codec_checked(
                &decrypted_xml,
                bir_rules::serialization::BodyCodec::Utf8PercentRfc3986Unreserved,
            )
            .expect("the generated transport payload should contain checked UTF-8 BIR XML"),
            queued.to_bir_field_map()
        );
    }

    #[tokio::test]
    async fn test_run_queue_tick_does_not_panic() {
        let db_file = NamedTempFile::new().unwrap();
        // Since we are just testing the runner, if keyring fails we can't test.
        let db = match Database::open(db_file.path()) {
            Ok(db) => db,
            Err(_) => return, // Skip test if keychain fails
        };
        let db = Arc::new(Mutex::new(db));

        // Just run the tick on an empty database. Should not panic.
        run_queue_tick(db.clone()).await;
    }

    #[test]
    fn submission_preparation_uses_current_profile_and_rejects_changes() {
        let mut reviewed_profile = reviewed_profile();
        reviewed_profile.eopt_tier = Some(EoptTier::Medium);
        reviewed_profile.profile_versions[0].eopt_tier = Some(EoptTier::Medium);
        let draft = queued_draft(&reviewed_profile);

        let mut current_profile = reviewed_profile;
        current_profile.eopt_tier = Some(EoptTier::Micro);
        current_profile.profile_versions[0].eopt_tier = Some(EoptTier::Micro);
        let prepared = prepare_queued_2551q(draft, &current_profile, None);

        match prepared {
            Queued2551QPreparation::Rejected { draft, errors, .. } => {
                assert_eq!(draft.status, FilingStatus::Draft);
                assert!(errors.iter().any(|(field, _)| field == "profile_snapshot"));
            }
            _ => panic!("a changed current profile must reject the queued return"),
        }
    }

    /// The cron's 2551Q submission boundary (`prepare_queued_2551q`) for a
    /// year profile with no confirmed version, and for one whose versions
    /// overlap and lack dates: the return is ready, nothing asks for a
    /// version.
    #[test]
    fn submission_preparation_needs_no_confirmed_profile_version() {
        let reviewed_profile = reviewed_profile();
        let draft = queued_draft(&reviewed_profile);
        let mut without = reviewed_profile.clone();
        without.profile_versions.clear();
        let mut messy = reviewed_profile;
        let mut overlapping = messy.profile_versions[0].clone();
        overlapping.id = "overlapping".into();
        overlapping.cor.registered_name = "Someone Else".into();
        let mut undated = overlapping.clone();
        undated.id = "undated".into();
        undated.effective_from = None;
        messy.profile_versions.push(overlapping);
        messy.profile_versions.push(undated);

        for current_profile in [without, messy] {
            match prepare_queued_2551q(draft.clone(), &current_profile, None) {
                Queued2551QPreparation::Ready { draft, .. } => {
                    assert_eq!(draft.status, FilingStatus::Queued);
                    assert!(draft.profile_resolution_error.is_none());
                    assert!(!draft.profile_snapshot_stale);
                }
                _ => panic!("the queued return needs no profile version"),
            }
        }
    }

    #[test]
    fn rejected_2551q_preparation_preserves_the_original_queue_revision() {
        let mut reviewed_profile = reviewed_profile();
        reviewed_profile.eopt_tier = Some(EoptTier::Medium);
        reviewed_profile.profile_versions[0].eopt_tier = Some(EoptTier::Medium);
        let mut draft = queued_draft(&reviewed_profile);
        draft.submission_attempts = 3;
        draft.next_retry_at = Some("2099-01-01T00:00:00Z".to_string());
        let expected_revision =
            queued_2551q_revision(&draft).expect("the unclaimed queue revision should exist");

        let mut current_profile = reviewed_profile;
        current_profile.eopt_tier = Some(EoptTier::Micro);
        current_profile.profile_versions[0].eopt_tier = Some(EoptTier::Micro);

        match prepare_queued_2551q(draft, &current_profile, None) {
            Queued2551QPreparation::Rejected {
                draft, revision, ..
            } => {
                assert_eq!(revision, expected_revision);
                assert_eq!(draft.status, FilingStatus::Draft);
                assert!(draft.queued_submission_fingerprint.is_none());
                assert!(draft.next_retry_at.is_none());
                assert_eq!(draft.submission_attempts, 0);
            }
            _ => panic!("a rejected preparation must retain its source queue revision"),
        }
    }

    #[test]
    fn submission_revision_rejects_cancel_and_identical_requeue() {
        let profile = reviewed_profile();
        let draft = queued_draft(&profile);
        let revision = match prepare_queued_2551q(draft.clone(), &profile, None) {
            Queued2551QPreparation::Ready { revision, .. } => revision,
            _ => panic!("the unchanged queued return must prepare"),
        };

        let mut canceled = draft.clone();
        canceled.revert_to_draft();
        assert!(matches!(
            prepare_queued_2551q(canceled, &profile, Some(&revision)),
            Queued2551QPreparation::Superseded
        ));

        let mut requeued = draft;
        requeued.next_retry_at = Some("2099-01-01T00:00:00Z".to_string());
        assert!(matches!(
            prepare_queued_2551q(requeued, &profile, Some(&revision)),
            Queued2551QPreparation::Superseded
        ));
    }

    #[test]
    fn unresolved_submission_claim_is_never_eligible_for_automatic_retry() {
        let profile = reviewed_profile();
        let mut draft = queued_draft(&profile);
        draft.submission_claim_token = Some("abandoned-network-claim".to_string());
        draft.submission_claimed_at = Some("2099-01-01T00:00:00Z".to_string());

        assert!(queued_2551q_revision(&draft).is_none());
        assert!(matches!(
            prepare_queued_2551q(draft, &profile, None),
            Queued2551QPreparation::Superseded
        ));
    }

    #[test]
    fn queued_1601c_preparation_binds_exact_fields_and_queue_generation() {
        let profile = test_profile();
        let draft = queued_1601c_draft(&profile);
        let revision = match prepare_queued_1601c(draft.clone(), None) {
            Queued1601CPreparation::Ready { draft, revision } => {
                assert!(draft.try_to_bir_xml_payload().is_ok());
                revision
            }
            _ => panic!("the unchanged 1601C queue snapshot should prepare"),
        };

        let mut tampered = draft.clone();
        tampered.tax_14_total_compensation = 10.0;
        match prepare_queued_1601c(tampered, Some(&revision)) {
            Queued1601CPreparation::Rejected { draft, errors, .. } => {
                assert_eq!(draft.status, FilingStatus::Draft);
                assert!(
                    errors
                        .iter()
                        .any(|(field, _)| field == "queued_submission_fingerprint")
                );
            }
            _ => panic!("tampered 1601C fields must fail before XML generation"),
        }

        let mut requeued = draft;
        requeued.next_retry_at = Some("2099-01-01T00:00:00Z".to_string());
        assert!(matches!(
            prepare_queued_1601c(requeued, Some(&revision)),
            Queued1601CPreparation::Superseded
        ));
    }

    #[test]
    fn unresolved_1601c_claim_is_never_eligible_for_automatic_retry() {
        let profile = test_profile();
        let mut draft = queued_1601c_draft(&profile);
        draft.submission_claim_token = Some("unknown-network-outcome".to_string());
        draft.submission_claimed_at = Some("2099-01-01T00:00:00Z".to_string());

        assert!(queued_1601c_revision(&draft).is_none());
        assert!(matches!(
            prepare_queued_1601c(draft, None),
            Queued1601CPreparation::Superseded
        ));
    }

    #[tokio::test]
    async fn process_queued_1601c_success_finalizes_exact_payload_and_schedules_email_poll() {
        let mut profile = test_profile();
        profile.email_tracking_enabled = true;
        profile.imap_email = Some("receipts@example.com".to_string());
        let queued = queued_1601c_draft(&profile);
        let db = Arc::new(Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        ));
        let id = db
            .lock()
            .expect("the cron database should not be poisoned")
            .save_queued_1601c_draft(&queued)
            .expect("the reviewed 1601C queue snapshot should persist");
        let summary = queued_1601c_summary(id, &queued);
        let transport = RecordingSubmissionTransport::new(TestTransportOutcome::Success);

        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &transport).await;

        let calls = transport.calls();
        assert_eq!(calls.len(), 1);
        assert_exact_1601c_transport_call(&calls[0], &queued);
        let db_guard = db.lock().expect("the cron database should not be poisoned");
        let submitted = db_guard
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .expect("the submitted 1601C lookup should succeed")
            .expect("the submitted 1601C snapshot should remain persisted");
        assert_eq!(submitted.status, FilingStatus::Submitted);
        assert_eq!(submitted.to_bir_field_map(), queued.to_bir_field_map());
        assert_eq!(
            submitted.submission_filename.as_deref(),
            Some(queued.default_submission_filename().as_str())
        );
        assert!(submitted.submitted_at.is_some());
        assert!(submitted.submission_claim_token.is_none());
        assert!(submitted.submission_claimed_at.is_none());
        assert!(!submitted.submitted_dry_run, "a real send is not a dry run");
        let jobs = db_guard
            .list_jobs()
            .expect("the email-poll job lookup should succeed");
        let email_job = jobs
            .iter()
            .find(|job| {
                job.name
                    == "Waiting for 1601C May 2024 confirmation for 123-456-789-000 (receipts@example.com)"
            })
            .expect("successful submission should schedule its confirmation email poll");
        assert_eq!(email_job.status, "Queued");
        assert_eq!(
            email_job.command.as_deref(),
            Some("bir_poll_email receipts@example.com")
        );
        drop(db_guard);
        let stored = stored_json(&db, "1601C", &queued.tin);
        assert!(stored.contains("\"submitted_at\""), "{stored}");
        assert!(
            !stored.contains("submitted_dry_run"),
            "a real send stores no dry-run key: {stored}"
        );
    }

    #[test]
    fn queued_1601c_attempt_masks_a_compact_13_digit_tin_in_the_log_file() {
        let profile = test_profile();
        let mut queued = Form1601CDraft::new_from_profile(&profile, 2024, 5);
        queued.tin = "1234567890000".to_string();
        queued.any_taxes_withheld = false;
        queued
            .transition_to_queued()
            .expect("a compact 13-digit submission TIN is accepted today");
        let db = Arc::new(Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        ));
        let id = db
            .lock()
            .expect("the cron database should not be poisoned")
            .save_queued_1601c_draft(&queued)
            .expect("the reviewed 1601C queue snapshot should persist");
        let summary = queued_1601c_summary(id, &queued);
        let transport = RecordingSubmissionTransport::new(TestTransportOutcome::Success);
        let dir = tempfile::tempdir().unwrap();
        let options = crate::logging::InstallOptions {
            developer_mode: false,
            default_filter: crate::logging::shell_filter(false),
        };

        temp_env::with_vars(
            [
                ("BIR_LOG_DIR", Some(dir.path().as_os_str())),
                ("RUST_LOG", None),
            ],
            || {
                crate::logging::with_thread_subscriber(options, || {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("the cron test runtime should build")
                        .block_on(process_queued_1601c_with_transport(
                            &summary, &profile, db, &transport,
                        ));
                });
            },
        );

        let path = crate::log_files::today_log_path(dir.path());
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        assert!(
            text.contains("Cron: Attempting to submit queued 1601C"),
            "{text}"
        );
        assert!(!text.contains("1234567890000"), "{text}");
        assert!(text.contains("***-***-789-0000"), "{text}");
    }

    #[test]
    fn relay_server_text_is_masked_in_the_cron_log_file() {
        let db = Arc::new(Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        ));
        let dir = tempfile::tempdir().unwrap();
        let options = crate::logging::InstallOptions {
            developer_mode: false,
            default_filter: crate::logging::shell_filter(false),
        };
        temp_env::with_vars(
            [
                ("BIR_LOG_DIR", Some(dir.path().as_os_str())),
                ("RUST_LOG", None),
            ],
            || {
                crate::logging::with_thread_subscriber(options, || {
                    release_unaccepted(
                        "1601C",
                        "052024",
                        &crate::transport::TransportError::Refused(
                            "TIN 1234567890000 refused; write payroll.team@example.com".into(),
                        ),
                        &db,
                        |_, _, _| Ok(true),
                    );
                });
            },
        );
        let path = crate::log_files::today_log_path(dir.path());
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        assert!(text.contains("was not sent"), "{text}");
        assert!(!text.contains("1234567890000"), "{text}");
        assert!(text.contains("***-***-789-0000"), "{text}");
        assert!(!text.contains("payroll.team@example.com"), "{text}");
        assert!(text.contains("pa•••@example.com"), "{text}");
    }

    #[tokio::test]
    async fn process_queued_1601c_dry_run_submits_without_email_poll() {
        let mut profile = test_profile();
        profile.email_tracking_enabled = true;
        profile.imap_email = Some("receipts@example.com".to_string());
        let queued = queued_1601c_draft(&profile);
        let db = Arc::new(Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        ));
        let id = db
            .lock()
            .expect("the cron database should not be poisoned")
            .save_queued_1601c_draft(&queued)
            .expect("the reviewed 1601C queue snapshot should persist");
        let summary = queued_1601c_summary(id, &queued);
        let transport = RecordingSubmissionTransport::dry_run(TestTransportOutcome::Success);

        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &transport).await;

        let submitted = db
            .lock()
            .expect("the cron database should not be poisoned")
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .expect("the submitted 1601C lookup should succeed")
            .expect("the submitted 1601C snapshot should remain persisted");
        assert_eq!(submitted.status, FilingStatus::Submitted);
        assert!(
            submitted.submitted_dry_run,
            "a dry-run send is stored as one"
        );
        assert!(stored_json(&db, "1601C", &queued.tin).contains("\"submitted_dry_run\":true"));
        let jobs = db
            .lock()
            .expect("the cron database should not be poisoned")
            .list_jobs()
            .expect("job lookup should succeed");
        assert!(
            jobs.iter().all(|job| !is_email_poll_job(job)),
            "dry-run must not schedule IMAP poll"
        );
    }

    #[tokio::test]
    async fn process_queued_1601c_unknown_outcome_remains_claimed_and_is_not_retried() {
        let profile = test_profile();
        let queued = queued_1601c_draft(&profile);
        let expected_fingerprint = queued.queued_submission_fingerprint.clone();
        let expected_retry_at = queued.next_retry_at.clone();
        let db = Arc::new(Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        ));
        let id = db
            .lock()
            .expect("the cron database should not be poisoned")
            .save_queued_1601c_draft(&queued)
            .expect("the reviewed 1601C queue snapshot should persist");
        let summary = queued_1601c_summary(id, &queued);
        let failing_transport = RecordingSubmissionTransport::new(TestTransportOutcome::UnknownIo);

        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &failing_transport)
            .await;

        let calls = failing_transport.calls();
        assert_eq!(calls.len(), 1);
        assert_exact_1601c_transport_call(&calls[0], &queued);
        let claimed = db
            .lock()
            .expect("the cron database should not be poisoned")
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .expect("the claimed 1601C lookup should succeed")
            .expect("the claimed 1601C snapshot should remain persisted");
        assert_eq!(claimed.status, FilingStatus::Queued);
        assert_eq!(claimed.queued_submission_fingerprint, expected_fingerprint);
        assert_eq!(claimed.next_retry_at, expected_retry_at);
        assert_eq!(claimed.submission_attempts, 0);
        assert!(claimed.submission_claim_token.is_some());
        assert!(claimed.submission_claimed_at.is_some());
        assert!(
            claimed
                .submission_error
                .as_deref()
                .is_some_and(|message| message.starts_with(OUTCOME_UNKNOWN)),
            "{:?}",
            claimed.submission_error
        );
        assert!(queued_1601c_revision(&claimed).is_none());

        let retry_transport = RecordingSubmissionTransport::new(TestTransportOutcome::Success);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &retry_transport).await;
        assert!(retry_transport.calls().is_empty());
        let still_claimed = db
            .lock()
            .expect("the cron database should not be poisoned")
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .expect("the unresolved 1601C lookup should succeed")
            .expect("the unresolved 1601C snapshot should remain persisted");
        assert_eq!(
            still_claimed.submission_claim_token,
            claimed.submission_claim_token
        );
        assert!(
            db.lock()
                .expect("the cron database should not be poisoned")
                .list_jobs()
                .expect("the email-poll job lookup should succeed")
                .is_empty()
        );
    }

    // ----- Generic QueueableForm worker -----

    use crate::forms::queueable::test_support::TestForm;

    fn allow_test_form(code: &str) -> Option<&'static str> {
        (code == TestForm::CODE).then_some(TestForm::TYPE)
    }

    fn queued_test_form(db: &Arc<Mutex<Database>>) -> (TestForm, FormDraftSummary) {
        let mut form = TestForm::new(6, 10.0);
        form.queue(crate::filing_queue::QueueAuthSource::Gui)
            .expect("the test form should queue");
        let id = db
            .lock()
            .unwrap()
            .save_queued_queueable(&form)
            .expect("the queued test form should persist");
        let summary = FormDraftSummary {
            id,
            tin: form.tin.clone(),
            form_code: TestForm::CODE.to_string(),
            taxable_year: form.taxable_year,
            quarter: None,
            month: Some(form.month),
            status: FilingStatus::Queued,
            updated_at: form.lifecycle.updated_at.clone(),
        };
        (form, summary)
    }

    fn stored_test_form(db: &Arc<Mutex<Database>>, form: &TestForm) -> TestForm {
        db.lock()
            .unwrap()
            .get_queueable_draft::<TestForm>(&form.tin, form.taxable_year, i64::from(form.month))
            .unwrap()
            .expect("the test form should stay persisted")
    }

    fn test_db() -> Arc<Mutex<Database>> {
        Arc::new(Mutex::new(Database::open_in_memory_for_tests().unwrap()))
    }

    #[tokio::test]
    async fn generic_worker_submits_the_exact_official_payload() {
        let profile = test_profile();
        let db = test_db();
        let (queued, summary) = queued_test_form(&db);
        let transport = RecordingSubmissionTransport::dry_run(TestTransportOutcome::Success);

        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &transport,
            allow_test_form,
        )
        .await;

        let calls = transport.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].form_type, TestForm::TYPE);
        assert_eq!(calls[0].filename, queued.submission_filename());
        let plain = crate::crypto::decrypt_and_decompress(
            &calls[0].payload,
            crate::crypto::BIR_IAF_PASSPHRASE,
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(plain).unwrap(),
            queued.official_payload().unwrap()
        );
        let submitted = stored_test_form(&db, &queued);
        assert_eq!(submitted.lifecycle.status, FilingStatus::Submitted);
        assert!(submitted.lifecycle.is_unclaimed());
        assert_eq!(
            submitted.lifecycle.submission_filename.as_deref(),
            Some(queued.submission_filename().as_str())
        );
        assert!(submitted.lifecycle.submitted_dry_run);
    }

    #[tokio::test]
    async fn generic_worker_real_send_stores_no_dry_run_key() {
        let profile = test_profile();
        let db = test_db();
        let (queued, summary) = queued_test_form(&db);
        let transport = RecordingSubmissionTransport::new(TestTransportOutcome::Success);

        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &transport,
            allow_test_form,
        )
        .await;

        assert_eq!(transport.calls().len(), 1);
        let submitted = stored_test_form(&db, &queued);
        assert_eq!(submitted.lifecycle.status, FilingStatus::Submitted);
        assert!(!submitted.lifecycle.submitted_dry_run);
        let stored = stored_json(&db, TestForm::CODE, &queued.tin);
        assert!(stored.contains("\"submitted_at\""), "{stored}");
        assert!(!stored.contains("submitted_dry_run"), "{stored}");
    }

    /// A build with nowhere to send (not a dry run, no mock, no `direct-bir`) leaves a queued return exactly as it
    /// is: no job starts for it, no attempt is counted and it does not go back to Draft.
    #[cfg(not(feature = "direct-bir"))]
    #[tokio::test]
    async fn a_build_with_nowhere_to_send_leaves_the_queue_alone() {
        let db = test_db();
        let (queued, _) = stored_queued_2551q(&db);
        let profile = reviewed_profile();
        let job = format!(
            "form:2551Q:{}:{}:{}",
            profile.tin.full(),
            queued.taxable_year,
            queued.quarter
        );
        temp_env::async_with_vars(
            [
                ("EBIR_TEST_ENV", None),
                ("BIR_SFTP_DRY_RUN", None),
                ("BIR_SFTP_HOST", None),
                (crate::transport::RELAY_URL_ENV, None),
                ("BIR_SFTP_LIVE", Some("1")),
            ],
            async {
                assert!(crate::transport::sending_unavailable());
                let waiting = db.lock().unwrap().list_all_queued_submissions().unwrap();
                assert_eq!(waiting.len(), 1, "the return is in the queue");
                process_submission_queue(&profile, db.clone()).await;
            },
        )
        .await;
        assert!(
            !get_active_jobs().lock().unwrap().contains(&job),
            "no job started"
        );
        let stored = db
            .lock()
            .unwrap()
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, FilingStatus::Queued);
        assert_eq!(stored.submission_attempts, 0);
        assert_eq!(stored.last_error, None);
    }

    /// A queued 2551Q saved with its profile, and the worker's summary of it.
    fn stored_queued_2551q(db: &Arc<Mutex<Database>>) -> (Form2551QDraft, FormDraftSummary) {
        let profile = reviewed_profile();
        let guard = db.lock().unwrap();
        guard.save_profile(profile.clone()).unwrap();
        let queued = queued_draft(&profile);
        let id = guard.save_queued_2551q_draft_and_election(&queued).unwrap();
        let summary = FormDraftSummary {
            id,
            tin: queued.tin.clone(),
            form_code: "2551Q".to_string(),
            taxable_year: queued.taxable_year,
            quarter: Some(queued.quarter),
            month: None,
            status: FilingStatus::Queued,
            updated_at: queued.updated_at.clone(),
        };
        (queued, summary)
    }

    #[test]
    fn the_dry_run_fact_of_1601c_and_2551q_describes_only_its_own_send() {
        let mut monthly = queued_1601c_draft(&test_profile());
        let reviewed = monthly.to_bir_field_map();
        let filename = monthly.default_submission_filename();
        monthly.transition_to_submitted(filename, true);
        assert!(monthly.submitted_dry_run);
        assert_eq!(monthly.to_bir_field_map(), reviewed);
        let mut legacy = serde_json::to_value(&monthly).unwrap();
        assert_eq!(legacy["submitted_dry_run"], true);
        legacy.as_object_mut().unwrap().remove("submitted_dry_run");
        assert!(
            !serde_json::from_value::<Form1601CDraft>(legacy)
                .unwrap()
                .submitted_dry_run
        );
        monthly.revert_to_draft();
        assert!(!monthly.submitted_dry_run);
        monthly.submitted_dry_run = true;
        monthly.transition_to_queued().unwrap();
        assert!(!monthly.submitted_dry_run);
        let filename = monthly.default_submission_filename();
        monthly.transition_to_submitted(filename, false);
        assert!(
            serde_json::to_value(&monthly)
                .unwrap()
                .get("submitted_dry_run")
                .is_none()
        );

        let mut quarterly = queued_draft(&reviewed_profile());
        let reviewed = quarterly.to_bir_field_map();
        let filename = quarterly.default_submission_filename();
        quarterly.transition_to_submitted(filename, true);
        assert!(quarterly.submitted_dry_run);
        assert_eq!(quarterly.to_bir_field_map(), reviewed);
        let mut legacy = serde_json::to_value(&quarterly).unwrap();
        assert_eq!(legacy["submitted_dry_run"], true);
        legacy.as_object_mut().unwrap().remove("submitted_dry_run");
        assert!(
            !serde_json::from_value::<Form2551QDraft>(legacy)
                .unwrap()
                .submitted_dry_run
        );
        quarterly.revert_to_draft();
        assert!(!quarterly.submitted_dry_run);
        quarterly.submitted_dry_run = true;
        quarterly.transition_to_queued().unwrap();
        assert!(!quarterly.submitted_dry_run);
        let filename = quarterly.default_submission_filename();
        quarterly.transition_to_submitted(filename, false);
        assert!(
            serde_json::to_value(&quarterly)
                .unwrap()
                .get("submitted_dry_run")
                .is_none()
        );
    }

    #[tokio::test]
    async fn process_queued_2551q_dry_run_is_stored_as_one() {
        let db = test_db();
        let (queued, summary) = stored_queued_2551q(&db);
        let transport = RecordingSubmissionTransport::dry_run(TestTransportOutcome::Success);

        process_queued_2551q_with_transport(&summary, &reviewed_profile(), db.clone(), &transport)
            .await;

        assert_eq!(transport.calls().len(), 1);
        let submitted = db
            .lock()
            .unwrap()
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(submitted.status, FilingStatus::Submitted);
        assert!(submitted.submitted_dry_run);
        assert!(stored_json(&db, "2551Q", &queued.tin).contains("\"submitted_dry_run\":true"));
        assert!(
            db.lock()
                .unwrap()
                .list_jobs()
                .unwrap()
                .iter()
                .all(|job| !is_email_poll_job(job)),
            "dry-run must not schedule IMAP poll"
        );
    }

    /// The session fails before any upload: nothing was sent, so the return
    /// stays Queued and unclaimed, with one attempt counted, to be retried.
    #[tokio::test]
    async fn process_queued_2551q_pre_store_failure_stays_unclaimed_and_backs_off() {
        let db = test_db();
        let (queued, summary) = stored_queued_2551q(&db);
        let transport = RecordingSubmissionTransport::new(TestTransportOutcome::PreStore);

        process_queued_2551q_with_transport(&summary, &reviewed_profile(), db.clone(), &transport)
            .await;

        assert!(transport.calls().is_empty());
        let failed = db
            .lock()
            .unwrap()
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(failed.status, FilingStatus::Queued);
        assert_eq!(failed.submission_attempts, 1);
        assert!(failed.submission_claim_token.is_none());
        assert!(failed.submission_put_started_at.is_none());
        assert!(failed.submitted_at.is_none());
        assert!(
            failed
                .last_error
                .as_deref()
                .is_some_and(|message| message.contains("no PUT attempted")),
            "{:?}",
            failed.last_error
        );
    }

    /// The upload's outcome is unknown (it may have reached BIR): the return
    /// keeps its claim and is never uploaded again, so it cannot be filed
    /// twice.
    #[tokio::test]
    async fn process_queued_2551q_unknown_outcome_stays_claimed_and_is_not_retried() {
        let db = test_db();
        let (queued, summary) = stored_queued_2551q(&db);
        let transport = RecordingSubmissionTransport::new(TestTransportOutcome::UnknownIo);

        process_queued_2551q_with_transport(&summary, &reviewed_profile(), db.clone(), &transport)
            .await;

        assert_eq!(transport.calls().len(), 1);
        let stored = || {
            db.lock()
                .unwrap()
                .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
                .unwrap()
                .unwrap()
        };
        let claimed = stored();
        assert_eq!(claimed.status, FilingStatus::Queued);
        assert!(claimed.submission_claim_token.is_some());
        assert!(claimed.submission_put_started_at.is_some());
        assert!(claimed.submitted_at.is_none());

        // A second run must not upload again: the outcome is unknown.
        let retry = RecordingSubmissionTransport::new(TestTransportOutcome::Success);
        process_queued_2551q_with_transport(&summary, &reviewed_profile(), db.clone(), &retry)
            .await;
        assert!(retry.calls().is_empty());
        let after = stored();
        assert_eq!(after.status, FilingStatus::Queued);
        assert!(after.submission_claim_token.is_some());
    }

    #[tokio::test]
    async fn process_queued_2551q_real_send_stores_no_dry_run_key() {
        let db = test_db();
        let (queued, summary) = stored_queued_2551q(&db);
        let transport = RecordingSubmissionTransport::new(TestTransportOutcome::Success);

        process_queued_2551q_with_transport(&summary, &reviewed_profile(), db.clone(), &transport)
            .await;

        assert_eq!(transport.calls().len(), 1);
        let submitted = db
            .lock()
            .unwrap()
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(submitted.status, FilingStatus::Submitted);
        assert!(!submitted.submitted_dry_run);
        let stored = stored_json(&db, "2551Q", &queued.tin);
        assert!(stored.contains("\"submitted_at\""), "{stored}");
        assert!(!stored.contains("submitted_dry_run"), "{stored}");
    }

    #[tokio::test]
    async fn generic_receipt_confirms_by_uploaded_filename_when_the_period_cannot_key_it() {
        let profile = test_profile();
        let db = test_db();
        let (queued, summary) = queued_test_form(&db);
        // A real send: a BIR email confirms only a return that reached the BIR.
        let transport = RecordingSubmissionTransport::new(TestTransportOutcome::Success);
        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &transport,
            allow_test_form,
        )
        .await;

        // BIR echoes the base filename; an event-based form's period part
        // (a date, a TCT) does not parse back to the draft's key.
        let local_now =
            chrono::Utc::now() + chrono::Duration::hours(8) + chrono::Duration::minutes(1);
        let uploaded = queued.submission_filename();
        let confirmation = crate::receipt::BirReceiptConfirmation {
            filename: format!("{}.xml", uploaded.split('#').next().unwrap()),
            date_received: local_now.date_naive(),
            time_received: local_now.time(),
            source_from: Some("ebirforms-noreply@bir.gov.ph".to_string()),
            raw_text: "This confirms receipt".to_string(),
            raw_html: None,
        };
        let guard = db.lock().unwrap();
        let (mut saved, _) = guard.save_submission_receipt(&confirmation).unwrap();
        saved.period = "03152025_T123456".to_string();
        assert_eq!(
            guard
                .confirm_queueable_from_receipt::<TestForm>(&saved)
                .unwrap(),
            crate::db::ReceiptConfirmationOutcome::Confirmed
        );
        drop(guard);
        assert_eq!(
            stored_test_form(&db, &queued).lifecycle.status,
            FilingStatus::Confirmed
        );
    }

    /// A dry run reached nobody: a BIR email naming its file (by period or by
    /// the uploaded filename) confirms nothing, and the receipt is Ignored as
    /// one with no matching return is.
    #[tokio::test]
    async fn a_bir_email_never_confirms_a_dry_run() {
        let profile = test_profile();
        let db = test_db();
        let (queued, summary) = queued_test_form(&db);
        let transport = RecordingSubmissionTransport::dry_run(TestTransportOutcome::Success);
        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &transport,
            allow_test_form,
        )
        .await;
        let sent = stored_test_form(&db, &queued);
        assert_eq!(sent.lifecycle.status, FilingStatus::Submitted);
        assert!(sent.lifecycle.submitted_dry_run);
        let local_now =
            chrono::Utc::now() + chrono::Duration::hours(8) + chrono::Duration::minutes(1);
        let uploaded = queued.submission_filename();
        let confirmation = crate::receipt::BirReceiptConfirmation {
            filename: format!("{}.xml", uploaded.split('#').next().unwrap()),
            date_received: local_now.date_naive(),
            time_received: local_now.time(),
            source_from: Some("ebirforms-noreply@bir.gov.ph".to_string()),
            raw_text: "This confirms receipt".to_string(),
            raw_html: None,
        };
        let guard = db.lock().unwrap();
        let (saved, _) = guard.save_submission_receipt(&confirmation).unwrap();
        for receipt in [saved.clone(), {
            let mut by_name = saved.clone();
            by_name.period = "03152025_T123456".to_string();
            by_name
        }] {
            assert_eq!(
                guard.confirm_submission_from_receipt(&receipt).unwrap(),
                crate::db::ReceiptConfirmationOutcome::Ignored
            );
        }
        drop(guard);
        let after = stored_test_form(&db, &queued);
        assert_eq!(after.lifecycle.status, FilingStatus::Submitted);
        assert!(after.lifecycle.receipt_id.is_none());
    }

    /// Dev Mode's simulated BIR email (debug builds) confirms a dry run on
    /// purpose: it is the simulation of the email a dry run never gets.
    #[tokio::test]
    async fn dev_mode_simulated_confirmation_uses_the_email_path() {
        let profile = test_profile();
        let db = test_db();
        let (queued, summary) = queued_test_form(&db);
        let transport = RecordingSubmissionTransport::dry_run(TestTransportOutcome::Success);
        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &transport,
            allow_test_form,
        )
        .await;
        let submitted = stored_test_form(&db, &queued);
        assert_eq!(submitted.lifecycle.status, FilingStatus::Submitted);
        let filename = submitted.lifecycle.submission_filename.clone().unwrap();
        let outcome =
            crate::dev_mode::simulate_bir_confirmation(&db.lock().unwrap(), &filename).unwrap();
        assert_eq!(outcome, crate::db::ReceiptConfirmationOutcome::Confirmed);
        let confirmed = stored_test_form(&db, &queued);
        assert_eq!(confirmed.lifecycle.status, FilingStatus::Confirmed);
        assert!(confirmed.lifecycle.receipt_id.is_some());
    }

    #[tokio::test]
    async fn generic_worker_pre_store_failure_stays_unclaimed_and_backs_off() {
        let profile = test_profile();
        let db = test_db();
        let (_queued, summary) = queued_test_form(&db);
        let transport = RecordingSubmissionTransport::new(TestTransportOutcome::PreStore);

        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &transport,
            allow_test_form,
        )
        .await;

        assert!(transport.calls().is_empty());
        let failed = stored_test_form(&db, &_queued);
        assert_eq!(failed.lifecycle.status, FilingStatus::Queued);
        assert_eq!(failed.lifecycle.submission_attempts, 1);
        assert!(failed.lifecycle.is_unclaimed());
        assert!(
            failed
                .lifecycle
                .submission_error
                .as_deref()
                .is_some_and(|message| message.contains("no PUT attempted"))
        );
    }

    #[tokio::test]
    async fn generic_worker_unknown_outcome_stays_claimed_and_is_not_retried() {
        let profile = test_profile();
        let db = test_db();
        let (queued, summary) = queued_test_form(&db);
        let transport = RecordingSubmissionTransport::new(TestTransportOutcome::UnknownIo);

        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &transport,
            allow_test_form,
        )
        .await;
        assert_eq!(transport.calls().len(), 1);
        let claimed = stored_test_form(&db, &queued);
        assert_eq!(claimed.lifecycle.status, FilingStatus::Queued);
        assert!(claimed.lifecycle.submission_put_started_at.is_some());
        assert!(!claimed.lifecycle.is_unclaimed());

        // A second run must not PUT again: the outcome is unknown.
        let retry = RecordingSubmissionTransport::new(TestTransportOutcome::Success);
        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &retry,
            allow_test_form,
        )
        .await;
        assert!(retry.calls().is_empty());
    }

    /// The back-off after a failure ends now: the stored return's `next_retry_at` moves into the past.
    fn make_retry_due(db: &Arc<Mutex<Database>>, form_code: &str, tin: &str) {
        let mut value: serde_json::Value =
            serde_json::from_str(&stored_json(db, form_code, tin)).unwrap();
        let past = json_retry_past();
        if value.get("lifecycle").is_some() {
            value["lifecycle"]["next_retry_at"] = past;
        } else {
            value["next_retry_at"] = past;
        }
        db.lock()
            .unwrap()
            .conn
            .execute(
                "UPDATE form_drafts SET data_json = ?1 WHERE form_code = ?2 AND tin = ?3",
                rusqlite::params![value.to_string(), form_code, tin],
            )
            .unwrap();
    }

    fn json_retry_past() -> serde_json::Value {
        serde_json::Value::String("1999-01-01T00:00:00Z".to_string())
    }

    /// A return queued while the worker was away has not missed a retry: it has never been tried. Its first
    /// send is due as soon as a worker can make it.
    #[tokio::test]
    async fn process_queued_1601c_first_send_after_restart_is_not_deferred() {
        let profile = test_profile();
        let (db, queued, summary) = stored_queued_1601c(&profile);
        let mut value: serde_json::Value =
            serde_json::from_str(&stored_json(&db, "1601C", &queued.tin)).unwrap();
        value["next_retry_at"] =
            serde_json::Value::String((Utc::now() - chrono::Duration::minutes(6)).to_rfc3339());
        db.lock()
            .unwrap()
            .conn
            .execute(
                "UPDATE form_drafts SET data_json = ?1 WHERE form_code = '1601C' AND tin = ?2",
                rusqlite::params![value.to_string(), queued.tin],
            )
            .unwrap();
        let transport = RecordingSubmissionTransport::relay(TestTransportOutcome::Success);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &transport).await;
        assert_eq!(transport.calls().len(), 1, "the first send is not a retry");
    }

    /// A retry missed while the worker was away is due the moment the worker wakes: it is sent in that
    /// same pass. The page never names its past stored time; `sendWords` says the try is happening now.
    #[tokio::test]
    async fn process_queued_1601c_missed_retry_is_sent_in_the_same_pass() {
        let profile = test_profile();
        let (db, queued, summary) = stored_queued_1601c(&profile);
        let mut value: serde_json::Value =
            serde_json::from_str(&stored_json(&db, "1601C", &queued.tin)).unwrap();
        value["next_retry_at"] = json_retry_past();
        value["submission_attempts"] = serde_json::json!(1);
        db.lock()
            .unwrap()
            .conn
            .execute(
                "UPDATE form_drafts SET data_json = ?1 WHERE form_code = '1601C' AND tin = ?2",
                rusqlite::params![value.to_string(), queued.tin],
            )
            .unwrap();
        let transport = RecordingSubmissionTransport::relay(TestTransportOutcome::Success);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &transport).await;
        assert_eq!(
            transport.calls().len(),
            1,
            "a missed retry is sent in the pass that finds it"
        );
        let sent = db
            .lock()
            .unwrap()
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        assert_eq!(sent.status, FilingStatus::Submitted);
    }

    fn stored_queued_1601c(
        profile: &TaxpayerProfile,
    ) -> (Arc<Mutex<Database>>, Form1601CDraft, FormDraftSummary) {
        let queued = queued_1601c_draft(profile);
        let db = Arc::new(Mutex::new(Database::open_in_memory_for_tests().unwrap()));
        let id = db.lock().unwrap().save_queued_1601c_draft(&queued).unwrap();
        let summary = queued_1601c_summary(id, &queued);
        (db, queued, summary)
    }

    fn assert_mock_submission_notice_and_no_poll(db: &Arc<Mutex<Database>>) {
        let db = db.lock().unwrap();
        assert!(
            db.list_jobs()
                .unwrap()
                .iter()
                .all(|job| !is_email_poll_job(job))
        );
        let alerts = db.list_active_alerts(None).unwrap();
        let notice = alerts
            .iter()
            .find(|alert| alert.kind.starts_with("form_submitted:"))
            .unwrap();
        assert_never_filed_alert(&notice.title, &notice.detail);
    }

    /// A 503 from the relay after the claim: it kept nothing, so the claim goes, the return stays Queued with a
    /// back-off, and once the relay is back it is sent and remembered as sent through the relay.
    #[tokio::test]
    async fn process_queued_1601c_relay_unavailable_releases_the_claim_and_retries() {
        let profile = test_profile();
        let (db, queued, summary) = stored_queued_1601c(&profile);
        let down = RecordingSubmissionTransport::relay(TestTransportOutcome::RelayUnavailable);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &down).await;
        assert_eq!(down.calls().len(), 1);
        let stored = || {
            db.lock()
                .unwrap()
                .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
                .unwrap()
                .unwrap()
        };
        let waiting = stored();
        assert_eq!(waiting.status, FilingStatus::Queued);
        assert!(waiting.submission_claim_token.is_none());
        assert!(waiting.submission_put_started_at.is_none());
        assert_eq!(waiting.submission_attempts, 1);
        assert!(waiting.next_retry_at.as_deref() > Some(Utc::now().to_rfc3339().as_str()));
        assert!(
            waiting
                .submission_error
                .as_deref()
                .is_some_and(|e| e.contains("unavailable")),
            "{:?}",
            waiting.submission_error
        );

        make_retry_due(&db, "1601C", &queued.tin);
        let back = RecordingSubmissionTransport::relay(TestTransportOutcome::Success);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &back).await;
        assert_eq!(back.calls().len(), 1);
        let sent = stored();
        assert_eq!(sent.status, FilingStatus::Submitted);
        assert!(sent.submitted_via_relay);
        assert!(!sent.submitted_dry_run);
        assert_mock_submission_notice_and_no_poll(&db);
    }

    /// A 4xx from the relay: the same file would be refused again, so the return goes back to Draft with the reason.
    #[tokio::test]
    async fn process_queued_1601c_relay_refusal_returns_it_to_draft_with_the_reason() {
        let profile = test_profile();
        let (db, queued, summary) = stored_queued_1601c(&profile);
        let refusing = RecordingSubmissionTransport::relay(TestTransportOutcome::RelayRefused);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &refusing).await;
        let draft = db
            .lock()
            .unwrap()
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        assert_eq!(draft.status, FilingStatus::Draft);
        assert!(draft.submission_claim_token.is_none());
        assert!(draft.submitted_at.is_none());
        assert!(
            draft
                .submission_error
                .as_deref()
                .is_some_and(|e| e.contains("refused")),
            "{:?}",
            draft.submission_error
        );
    }

    /// A send that did not go through the relay is not remembered as one.
    #[tokio::test]
    async fn process_queued_1601c_direct_send_is_not_marked_as_relay() {
        let profile = test_profile();
        let (db, queued, summary) = stored_queued_1601c(&profile);
        let direct = RecordingSubmissionTransport::new(TestTransportOutcome::Success);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &direct).await;
        let sent = db
            .lock()
            .unwrap()
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        assert_eq!(sent.status, FilingStatus::Submitted);
        assert!(!sent.submitted_via_relay);
        assert!(!stored_json(&db, "1601C", &queued.tin).contains("submitted_via_relay"));
    }

    /// 2551Q uses the same queue clock as 1601C: a retry missed while the worker was away is due now.
    #[tokio::test]
    async fn process_queued_2551q_missed_retry_is_sent_in_the_same_pass() {
        let db = test_db();
        let (queued, summary) = stored_queued_2551q(&db);
        let mut value: serde_json::Value =
            serde_json::from_str(&stored_json(&db, "2551Q", &queued.tin)).unwrap();
        value["next_retry_at"] = json_retry_past();
        value["submission_attempts"] = serde_json::json!(1);
        db.lock()
            .unwrap()
            .conn
            .execute(
                "UPDATE form_drafts SET data_json = ?1 WHERE form_code = '2551Q' AND tin = ?2",
                rusqlite::params![value.to_string(), queued.tin],
            )
            .unwrap();
        let transport = RecordingSubmissionTransport::relay(TestTransportOutcome::Success);
        process_queued_2551q_with_transport(&summary, &reviewed_profile(), db.clone(), &transport)
            .await;
        assert_eq!(
            transport.calls().len(),
            1,
            "a missed 2551Q retry is sent in the pass that finds it"
        );
        let sent = db
            .lock()
            .unwrap()
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(sent.status, FilingStatus::Submitted);
    }

    #[tokio::test]
    async fn process_queued_2551q_relay_unavailable_releases_the_claim_and_retries() {
        let db = test_db();
        let (queued, summary) = stored_queued_2551q(&db);
        let down = RecordingSubmissionTransport::relay(TestTransportOutcome::RelayUnavailable);
        process_queued_2551q_with_transport(&summary, &reviewed_profile(), db.clone(), &down).await;
        let stored = || {
            db.lock()
                .unwrap()
                .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
                .unwrap()
                .unwrap()
        };
        let waiting = stored();
        assert_eq!(waiting.status, FilingStatus::Queued);
        assert!(waiting.submission_claim_token.is_none());
        assert!(waiting.submission_put_started_at.is_none());
        assert_eq!(waiting.submission_attempts, 1);
        assert!(
            waiting
                .last_error
                .as_deref()
                .is_some_and(|e| e.contains("unavailable"))
        );

        make_retry_due(&db, "2551Q", &queued.tin);
        let back = RecordingSubmissionTransport::relay(TestTransportOutcome::Success);
        process_queued_2551q_with_transport(&summary, &reviewed_profile(), db.clone(), &back).await;
        let sent = stored();
        assert_eq!(sent.status, FilingStatus::Submitted);
        assert!(sent.submitted_via_relay);
        assert_mock_submission_notice_and_no_poll(&db);
    }

    #[tokio::test]
    async fn process_queued_2551q_relay_refusal_returns_it_to_draft_with_the_reason() {
        let db = test_db();
        let (queued, summary) = stored_queued_2551q(&db);
        let refusing = RecordingSubmissionTransport::relay(TestTransportOutcome::RelayRefused);
        process_queued_2551q_with_transport(&summary, &reviewed_profile(), db.clone(), &refusing)
            .await;
        let draft = db
            .lock()
            .unwrap()
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(draft.status, FilingStatus::Draft);
        assert!(draft.submission_claim_token.is_none());
        assert!(
            draft
                .last_error
                .as_deref()
                .is_some_and(|e| e.contains("refused"))
        );
    }

    /// The generic queueable processor uses the same queue clock: a missed retry is not deferred.
    #[tokio::test]
    async fn generic_worker_missed_retry_is_sent_in_the_same_pass() {
        let profile = test_profile();
        let db = test_db();
        let (queued, summary) = queued_test_form(&db);
        let mut value: serde_json::Value =
            serde_json::from_str(&stored_json(&db, TestForm::CODE, queued.tin())).unwrap();
        value["next_retry_at"] = json_retry_past();
        value["submission_attempts"] = serde_json::json!(1);
        db.lock()
            .unwrap()
            .conn
            .execute(
                "UPDATE form_drafts SET data_json = ?1 WHERE form_code = ?2 AND tin = ?3",
                rusqlite::params![value.to_string(), TestForm::CODE, queued.tin()],
            )
            .unwrap();
        let transport = RecordingSubmissionTransport::relay(TestTransportOutcome::Success);
        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &transport,
            allow_test_form,
        )
        .await;
        assert_eq!(
            transport.calls().len(),
            1,
            "a missed generic retry is sent in the pass that finds it"
        );
        let sent = stored_test_form(&db, &queued);
        assert_eq!(sent.lifecycle.status, FilingStatus::Submitted);
    }

    #[tokio::test]
    async fn generic_worker_relay_unavailable_releases_the_claim_and_a_refusal_returns_to_draft() {
        let profile = test_profile();
        let db = test_db();
        let (queued, summary) = queued_test_form(&db);
        let down = RecordingSubmissionTransport::relay(TestTransportOutcome::RelayUnavailable);
        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &down,
            allow_test_form,
        )
        .await;
        let waiting = stored_test_form(&db, &queued);
        assert_eq!(waiting.lifecycle.status, FilingStatus::Queued);
        assert!(waiting.lifecycle.is_unclaimed());
        assert!(waiting.lifecycle.submission_put_started_at.is_none());
        assert_eq!(waiting.lifecycle.submission_attempts, 1);

        make_retry_due(&db, TestForm::FORM_CODE, queued.tin());
        let back = RecordingSubmissionTransport::relay(TestTransportOutcome::Success);
        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &back,
            allow_test_form,
        )
        .await;
        let sent = stored_test_form(&db, &queued);
        assert_eq!(sent.lifecycle.status, FilingStatus::Submitted);
        assert!(sent.lifecycle.submitted_via_relay);
        assert_mock_submission_notice_and_no_poll(&db);

        let db = test_db();
        let (queued, summary) = queued_test_form(&db);
        let refusing = RecordingSubmissionTransport::relay(TestTransportOutcome::RelayRefused);
        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &refusing,
            allow_test_form,
        )
        .await;
        let draft = stored_test_form(&db, &queued);
        assert_eq!(draft.lifecycle.status, FilingStatus::Draft);
        assert!(draft.lifecycle.is_unclaimed());
        assert!(
            draft
                .lifecycle
                .submission_error
                .as_deref()
                .is_some_and(|e| e.contains("refused"))
        );
    }

    // ===== The Buwiz relay's outcomes, the same on the three send paths =====

    /// What these tests read off a return after one send.
    #[derive(Debug)]
    struct SendState {
        status: FilingStatus,
        claimed: bool,
        attempts: u32,
        error: Option<String>,
        next_retry_at: Option<chrono::DateTime<Utc>>,
        relay_receipt: Option<String>,
        dry_run: bool,
        /// The encrypted files handed to the transport, and the plain returns beside them.
        sent: Vec<(Vec<u8>, Vec<u8>)>,
        poll_jobs: usize,
        /// A second run of the worker with a transport that would succeed: how many sends it made.
        sent_again: usize,
        /// The submission notice's title and detail, as the worker kept them in its alert.
        notice: Option<(String, String)>,
    }

    fn submission_alert(db: &Arc<Mutex<Database>>) -> Option<(String, String)> {
        db.lock()
            .unwrap()
            .list_active_alerts(None)
            .unwrap()
            .into_iter()
            .find(|alert| alert.kind.starts_with("form_submitted:"))
            .map(|alert| (alert.title, alert.detail))
    }

    fn retry_time(raw: Option<&str>) -> Option<chrono::DateTime<Utc>> {
        raw.map(|raw| {
            chrono::DateTime::parse_from_rfc3339(raw)
                .expect("a retry time")
                .with_timezone(&Utc)
        })
    }

    fn sent_by(transport: &RecordingSubmissionTransport) -> Vec<(Vec<u8>, Vec<u8>)> {
        let plain = transport.plain.lock().unwrap().clone();
        transport
            .calls()
            .into_iter()
            .map(|call| call.payload)
            .zip(plain)
            .collect()
    }

    fn poll_jobs(db: &Arc<Mutex<Database>>) -> usize {
        db.lock()
            .unwrap()
            .list_jobs()
            .unwrap()
            .iter()
            .filter(|job| is_email_poll_job(job))
            .count()
    }

    async fn buwiz_1601c(outcome: TestTransportOutcome) -> SendState {
        let profile = test_profile();
        let (db, queued, summary) = stored_queued_1601c(&profile);
        let transport = RecordingSubmissionTransport::buwiz(outcome);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &transport).await;
        let again = RecordingSubmissionTransport::buwiz(TestTransportOutcome::BuwizSent);
        let read = |db: &Arc<Mutex<Database>>| {
            db.lock()
                .unwrap()
                .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
                .unwrap()
                .unwrap()
        };
        let draft = read(&db);
        let jobs = poll_jobs(&db);
        if matches!(draft.status, FilingStatus::Queued) {
            process_queued_1601c_with_transport(&summary, &profile, db.clone(), &again).await;
        }
        SendState {
            status: draft.status.clone(),
            claimed: draft.submission_claim_token.is_some(),
            attempts: draft.submission_attempts,
            error: draft.submission_error.clone(),
            next_retry_at: retry_time(draft.next_retry_at.as_deref()),
            relay_receipt: draft.relay_receipt.clone(),
            dry_run: draft.submitted_dry_run,
            sent: sent_by(&transport),
            poll_jobs: jobs,
            sent_again: again.calls().len(),
            notice: submission_alert(&db),
        }
    }

    async fn buwiz_2551q(outcome: TestTransportOutcome) -> SendState {
        let db = test_db();
        let (queued, summary) = stored_queued_2551q(&db);
        let profile = reviewed_profile();
        let transport = RecordingSubmissionTransport::buwiz(outcome);
        process_queued_2551q_with_transport(&summary, &profile, db.clone(), &transport).await;
        let again = RecordingSubmissionTransport::buwiz(TestTransportOutcome::BuwizSent);
        let draft = db
            .lock()
            .unwrap()
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        let jobs = poll_jobs(&db);
        if matches!(draft.status, FilingStatus::Queued) {
            process_queued_2551q_with_transport(&summary, &profile, db.clone(), &again).await;
        }
        SendState {
            status: draft.status.clone(),
            claimed: draft.submission_claim_token.is_some(),
            attempts: draft.submission_attempts,
            error: draft.last_error.clone(),
            next_retry_at: retry_time(draft.next_retry_at.as_deref()),
            relay_receipt: draft.relay_receipt.clone(),
            dry_run: draft.submitted_dry_run,
            sent: sent_by(&transport),
            poll_jobs: jobs,
            sent_again: again.calls().len(),
            notice: submission_alert(&db),
        }
    }

    async fn buwiz_generic(outcome: TestTransportOutcome) -> SendState {
        let profile = test_profile();
        let db = test_db();
        let (queued, summary) = queued_test_form(&db);
        let transport = RecordingSubmissionTransport::buwiz(outcome);
        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &transport,
            allow_test_form,
        )
        .await;
        let again = RecordingSubmissionTransport::buwiz(TestTransportOutcome::BuwizSent);
        let draft = stored_test_form(&db, &queued);
        let jobs = poll_jobs(&db);
        if matches!(draft.lifecycle.status, FilingStatus::Queued) {
            process_queued_queueable::<TestForm, _>(
                &summary,
                &profile,
                db.clone(),
                &again,
                allow_test_form,
            )
            .await;
        }
        SendState {
            status: draft.lifecycle.status.clone(),
            claimed: !draft.lifecycle.is_unclaimed(),
            attempts: draft.lifecycle.submission_attempts,
            error: draft.lifecycle.submission_error.clone(),
            next_retry_at: retry_time(draft.lifecycle.next_retry_at.as_deref()),
            relay_receipt: draft.lifecycle.relay_receipt.clone(),
            dry_run: draft.lifecycle.submitted_dry_run,
            sent: sent_by(&transport),
            poll_jobs: jobs,
            sent_again: again.calls().len(),
            notice: submission_alert(&db),
        }
    }

    /// One send with `outcome` on each of the three send paths.
    async fn buwiz_on_every_path(outcome: TestTransportOutcome) -> Vec<(&'static str, SendState)> {
        vec![
            ("1601C", buwiz_1601c(outcome).await),
            ("2551Q", buwiz_2551q(outcome).await),
            ("generic", buwiz_generic(outcome).await),
        ]
    }

    fn minutes_from_now(time: Option<chrono::DateTime<Utc>>) -> i64 {
        (time.expect("a retry time") - Utc::now()).num_minutes()
    }

    #[tokio::test]
    async fn a_relay_receipt_makes_the_return_submitted_and_is_kept() {
        for (path, state) in buwiz_on_every_path(TestTransportOutcome::BuwizSent).await {
            assert_eq!(state.status, FilingStatus::Submitted, "{path}");
            assert_eq!(
                state.relay_receipt.as_deref(),
                Some(BUWIZ_RECEIPT),
                "{path}"
            );
            assert!(!state.dry_run, "{path}: a real send");
            assert!(!state.claimed, "{path}");
            assert_eq!(state.error, None, "{path}");
            // The relay gets the plain return, the very bytes the encrypted file was made from.
            assert_eq!(state.sent.len(), 1, "{path}");
            let (encrypted, plain) = &state.sent[0];
            assert!(plain.starts_with(b"<?xml"), "{path}");
            assert_eq!(
                &crate::crypto::decrypt_and_decompress(
                    encrypted,
                    crate::crypto::BIR_IAF_PASSPHRASE
                )
                .unwrap(),
                plain,
                "{path}"
            );
        }
    }

    #[tokio::test]
    async fn a_receipt_for_a_simulated_send_is_a_dry_run_and_waits_for_no_email() {
        for (path, state) in buwiz_on_every_path(TestTransportOutcome::BuwizSimulated).await {
            assert_eq!(state.status, FilingStatus::Submitted, "{path}");
            assert!(state.dry_run, "{path}: nothing reached the BIR");
            assert_eq!(
                state.relay_receipt.as_deref(),
                Some(BUWIZ_RECEIPT),
                "{path}"
            );
            assert_eq!(state.poll_jobs, 0, "{path}: no BIR email is coming");
        }
    }

    /// Link 2 of 3 of "a relay dry_run result never reads as filed" (link 1: bir-submit
    /// `client::tests::a_relay_dry_run_result_with_its_signed_receipt_is_a_verified_test_send`; link 3: bir-app-api
    /// `screen_form_module::tests::a_relay_dry_run_result_never_reads_as_filed`). The shared fixture's result, as the
    /// client hands it over, through the real `send_outcome` and the real worker on each send path: Submitted as a dry
    /// run with its receipt, no BIR email watched, never sent again, and a notice that says the test-send sentence
    /// and never submitted, sent or filed.
    #[tokio::test]
    async fn a_relay_dry_run_result_is_stored_as_a_test_send_on_every_path() {
        let report = crate::relay::dry_run_result_fixture_report();
        assert!(report.simulated, "the signed claims say dry_run");
        let fixture: serde_json::Value =
            serde_json::from_str(crate::relay::DRY_RUN_RESULT_FIXTURE).unwrap();
        let receipt = report.relay_receipt.clone().expect("a receipt");
        assert_eq!(receipt_claims(&receipt), fixture["receipt_claims"]);
        for (path, state) in buwiz_on_every_path(TestTransportOutcome::BuwizDryRunResult).await {
            assert_eq!(state.status, FilingStatus::Submitted, "{path}");
            assert!(state.dry_run, "{path}: nothing reached the BIR");
            assert_eq!(state.relay_receipt.as_ref(), Some(&receipt), "{path}");
            assert!(!state.claimed, "{path}");
            assert_eq!(state.error, None, "{path}");
            assert_eq!(state.poll_jobs, 0, "{path}: no BIR email is coming");
            assert_eq!(state.sent_again, 0, "{path}");
            let (title, detail) = state.notice.expect("the submission notice");
            assert_never_filed_alert(&title, &detail);
        }
    }

    #[tokio::test]
    async fn a_relay_refusal_returns_the_return_to_draft_with_the_servers_sentence() {
        for (path, state) in buwiz_on_every_path(TestTransportOutcome::BuwizRefused).await {
            assert_eq!(state.status, FilingStatus::Draft, "{path}");
            assert!(!state.claimed, "{path}");
            assert_eq!(state.error.as_deref(), Some(BUWIZ_REFUSAL), "{path}");
            assert_eq!(state.attempts, 0, "{path}");
            assert_eq!(state.relay_receipt, None, "{path}");
        }
    }

    #[tokio::test]
    async fn a_failed_relay_try_stays_queued_and_waits_five_minutes() {
        for (path, state) in buwiz_on_every_path(TestTransportOutcome::BuwizFailed).await {
            assert_eq!(state.status, FilingStatus::Queued, "{path}");
            assert!(!state.claimed, "{path}: the claim is let go");
            assert_eq!(state.attempts, 1, "{path}");
            assert!(
                (4..=5).contains(&minutes_from_now(state.next_retry_at)),
                "{path}"
            );
            assert_eq!(state.sent_again, 0, "{path}: not before its time");
        }
    }

    #[tokio::test]
    async fn a_relay_hold_is_not_a_try_and_waits_as_long_as_the_server_says_at_most_a_week() {
        for (path, state) in buwiz_on_every_path(TestTransportOutcome::BuwizHeld).await {
            assert_eq!(state.status, FilingStatus::Queued, "{path}");
            assert!(!state.claimed, "{path}: the claim is let go");
            assert_eq!(state.attempts, 0, "{path}: nothing is counted");
            assert_eq!(state.error.as_deref(), Some(BUWIZ_HOLD), "{path}");
            let minutes = minutes_from_now(state.next_retry_at);
            assert!(
                (7 * 24 * 60 - 2..=7 * 24 * 60).contains(&minutes),
                "{path}: {minutes}"
            );
            assert_eq!(state.sent_again, 0, "{path}: not before its time");
        }
    }

    #[tokio::test]
    async fn buwiz_not_ready_before_the_claim_is_not_a_try_and_sends_nothing() {
        for (path, state) in buwiz_on_every_path(TestTransportOutcome::BuwizHeldBeforeClaim).await {
            assert_eq!(state.status, FilingStatus::Queued, "{path}");
            assert!(!state.claimed, "{path}");
            assert_eq!(state.attempts, 0, "{path}: nothing is counted");
            assert_eq!(
                state.error.as_deref(),
                Some("Sign in to send returns."),
                "{path}"
            );
            assert!(state.sent.is_empty(), "{path}");
            assert!(
                (4..=5).contains(&minutes_from_now(state.next_retry_at)),
                "{path}"
            );
        }
    }

    #[tokio::test]
    async fn an_unknown_relay_outcome_keeps_the_claim_says_why_and_is_never_sent_again() {
        for (path, state) in buwiz_on_every_path(TestTransportOutcome::BuwizUnknown).await {
            assert_eq!(state.status, FilingStatus::Queued, "{path}");
            assert!(state.claimed, "{path}: the claim stays");
            assert_eq!(state.attempts, 0, "{path}");
            let error = state.error.as_deref().unwrap_or_default();
            assert!(error.starts_with(OUTCOME_UNKNOWN), "{path}: {error}");
            assert!(
                error.contains("The relay ended without a result."),
                "{path}: {error}"
            );
            assert_eq!(state.sent.len(), 1, "{path}");
            assert_eq!(
                state.sent_again, 0,
                "{path}: the worker never sends it again"
            );
        }
    }

    /// A BIR outage must not empty the queue: however often the relay fails, the return stays Queued, on the long
    /// ladder (5 minutes, 30 minutes, 1 hour, then every 2 hours).
    #[test]
    fn relay_failures_never_send_a_return_back_to_draft() {
        use crate::forms::queueable::relay_retry_delay;
        let waits: Vec<i64> = (1..=8)
            .map(|n| relay_retry_delay(n).num_minutes())
            .collect();
        assert_eq!(waits, [5, 30, 60, 120, 120, 120, 120, 120]);

        let profile = test_profile();
        let mut monthly = queued_1601c_draft(&profile);
        let mut quarterly = queued_draft(&reviewed_profile());
        let mut generic = TestForm::new(6, 10.0);
        generic
            .queue(crate::filing_queue::QueueAuthSource::Gui)
            .unwrap();
        for _ in 0..12 {
            monthly.record_relay_failure("down".into());
            quarterly.record_relay_failure("down".into());
            generic.lifecycle.record_relay_failure("down".into());
        }
        assert_eq!(monthly.status, FilingStatus::Queued);
        assert_eq!(quarterly.status, FilingStatus::Queued);
        assert_eq!(generic.lifecycle.status, FilingStatus::Queued);
        assert_eq!(monthly.submission_attempts, 12);
        assert!(monthly.queued_submission_fingerprint.is_some());
        assert!(quarterly.queued_submission_fingerprint.is_some());
        assert!(generic.lifecycle.queued_submission_fingerprint.is_some());
    }

    /// The first run with relay filing gives back what was queued before it: each unclaimed Queued return goes to
    /// Draft with the reason, on all three storage shapes. It happens once: a return queued afterwards stays
    /// queued, and a claimed return (an unknown outcome) is never touched.
    #[test]
    fn returns_queued_before_relay_filing_go_back_to_draft_once() {
        let profile = test_profile();
        let (db, monthly, _) = stored_queued_1601c(&profile);
        let (quarterly, _) = stored_queued_2551q(&db);
        let (generic, generic_summary) = queued_test_form(&db);
        assert_eq!(
            db.lock().unwrap().get_setting(RELAY_SINCE_SETTING).unwrap(),
            None
        );

        give_back_returns_queued_before_the_relay(&db);

        let read_monthly = |db: &Arc<Mutex<Database>>| {
            db.lock()
                .unwrap()
                .get_1601c_draft(&monthly.tin, monthly.taxable_year, monthly.month)
                .unwrap()
                .unwrap()
        };
        let back = read_monthly(&db);
        assert_eq!(back.status, FilingStatus::Draft);
        assert_eq!(back.submission_error.as_deref(), Some(QUEUED_BEFORE_RELAY));
        assert!(back.queued_submission_fingerprint.is_none());
        let back = db
            .lock()
            .unwrap()
            .get_2551q_draft(&quarterly.tin, quarterly.taxable_year, quarterly.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(back.status, FilingStatus::Draft);
        assert_eq!(back.last_error.as_deref(), Some(QUEUED_BEFORE_RELAY));
        // The test form is no registered kind, so the sweep's own dispatch passes it by; its generic step is the
        // one every registered form goes through.
        give_back_queueable::<TestForm>(&generic_summary, &db, QUEUED_BEFORE_RELAY.to_string());
        let back = stored_test_form(&db, &generic);
        assert_eq!(back.lifecycle.status, FilingStatus::Draft);
        assert_eq!(
            back.lifecycle.submission_error.as_deref(),
            Some(QUEUED_BEFORE_RELAY)
        );
        assert!(
            db.lock()
                .unwrap()
                .get_setting(RELAY_SINCE_SETTING)
                .unwrap()
                .is_some()
        );

        // Once: what is queued after the first run is left to the worker.
        let (db_again, monthly_again, _) = stored_queued_1601c(&profile);
        db_again
            .lock()
            .unwrap()
            .set_setting(RELAY_SINCE_SETTING, "2026-11-25T00:00:00Z")
            .unwrap();
        give_back_returns_queued_before_the_relay(&db_again);
        let kept = db_again
            .lock()
            .unwrap()
            .get_1601c_draft(
                &monthly_again.tin,
                monthly_again.taxable_year,
                monthly_again.month,
            )
            .unwrap()
            .unwrap();
        assert_eq!(kept.status, FilingStatus::Queued);
        assert_eq!(kept.submission_error, None);
    }

    /// A claimed return has an unknown outcome: the sweep leaves it for the person to settle.
    #[tokio::test]
    async fn the_first_run_sweep_never_touches_a_return_with_an_unknown_outcome() {
        let profile = test_profile();
        let (db, queued, summary) = stored_queued_1601c(&profile);
        let unknown = RecordingSubmissionTransport::buwiz(TestTransportOutcome::BuwizUnknown);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &unknown).await;

        give_back_returns_queued_before_the_relay(&db);

        let stuck = db
            .lock()
            .unwrap()
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        assert_eq!(stuck.status, FilingStatus::Queued);
        assert!(stuck.submission_claim_token.is_some(), "the claim stays");
        assert!(
            stuck
                .submission_error
                .as_deref()
                .is_some_and(|reason| reason.starts_with(OUTCOME_UNKNOWN))
        );
    }

    // ===== End to end against a local Buwiz server in dry-run mode =====

    /// Sign in to the local server as its test account without a browser: start the device flow, approve the code
    /// with the test account's web session (the cookie line in `cookie_file`), poll, and save the sign-in.
    fn local_sign_in(server: &str, cookie_file: &str) {
        use crate::cloud_account;
        let mut http = cloud_account::BlockingHttp::new();
        let now = cloud_account::unix_now();
        let challenge =
            cloud_account::start_device(&mut http, server, now, &crate::install_id::install_id())
                .expect("device start");
        let cookie = std::fs::read_to_string(cookie_file).expect("the test session cookie");
        let approved = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
            .post(format!("{server}/device/decision"))
            .header("origin", server)
            .header("cookie", cookie.trim())
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "user_code={}&decision=approve",
                challenge.user_code
            ))
            .send()
            .expect("device approval");
        assert_eq!(approved.status().as_u16(), 303, "the approval redirects");
        let tokens = cloud_account::poll_until(
            &mut http,
            &challenge,
            &mut cloud_account::unix_now,
            &mut |delay| std::thread::sleep(delay),
        )
        .expect("device poll");
        let (user_id, email, practice) =
            cloud_account::load_profile(&mut http, server, &tokens.access_token).expect("/me");
        assert!(!practice, "the test account is not a practice account");
        let mut stored =
            cloud_account::StoredAccount::from_secrets(&cloud_account::AccountSecrets {
                server_url: server.to_owned(),
                access_token: tokens.access_token,
                refresh_token: tokens.refresh_token,
                access_expires_at_unix: now + tokens.expires_in,
                user_id,
                email,
                offline: false,
                practice,
            });
        stored.last_refresh_unix = now;
        cloud_account::save_stored_account(&stored).expect("save the sign-in");
    }

    /// The claims of a relay receipt, unverified (the client verified it before the queue kept it).
    fn receipt_claims(jws: &str) -> serde_json::Value {
        let payload = jws.split('.').nth(1).expect("a compact JWS");
        let bytes = data_encoding::BASE64URL_NOPAD
            .decode(payload.as_bytes())
            .expect("base64url");
        serde_json::from_slice(&bytes).expect("receipt claims")
    }

    /// The real worker, the real client and a real (local, dry-run) Buwiz server: sign in, register the device, send
    /// a 1601C and get a signed dry-run receipt; rotate the token and send a 2551Q; send a return for a TIN that is
    /// not on the account and see it refused back to Draft with the server's code.
    ///
    /// Needs the server the buwiz-com session runs (`BUWIZ_SERVER_URL`, on this computer), its test trust key
    /// (`BUWIZ_TRUST_KEYS`) and the test account's web session cookie file (`RELAY_E2E_COOKIE_FILE`). Run with
    /// `-- --ignored --exact background_cron::tests::relay_end_to_end_against_a_local_dry_run_server --nocapture`.
    #[test]
    #[ignore = "needs a local Buwiz server in dry-run mode"]
    fn relay_end_to_end_against_a_local_dry_run_server() {
        let server = std::env::var("BUWIZ_SERVER_URL").expect("BUWIZ_SERVER_URL");
        let cookie_file = std::env::var("RELAY_E2E_COOKIE_FILE").expect("RELAY_E2E_COOKIE_FILE");
        assert!(
            std::env::var("BUWIZ_TRUST_KEYS").is_ok(),
            "BUWIZ_TRUST_KEYS"
        );
        let scratch = tempfile::tempdir().unwrap();
        let database = scratch.path().join("bir_data.db");
        let account = scratch.path().join("account.json");
        temp_env::with_vars(
            [
                ("BIR_DATABASE_PATH", Some(database.to_str().unwrap())),
                (
                    crate::cloud_account::ENV_ACCOUNT_FILE,
                    Some(account.to_str().unwrap()),
                ),
                (crate::platform::NO_KEYCHAIN_ENV, Some("1")),
                (crate::transport::RELAY_SEND_ENV, Some("1")),
                ("EBIR_TEST_ENV", Some("1")),
                ("BIR_SFTP_DRY_RUN", Some("1")),
                ("BIR_SFTP_LIVE", None),
                ("BIR_SFTP_HOST", None),
                (crate::transport::RELAY_URL_ENV, None),
            ],
            || {
                assert!(
                    crate::transport::sends_through_buwiz(),
                    "the relay is the route"
                );
                local_sign_in(&server, &cookie_file);

                // The send future is deep: the worker's threads have 16 MiB stacks, and so does this one.
                std::thread::Builder::new()
                    .stack_size(32 * 1024 * 1024)
                    .spawn(|| {
                        tokio::runtime::Builder::new_multi_thread()
                            .enable_all()
                            .thread_stack_size(16 * 1024 * 1024)
                            .build()
                            .unwrap()
                            .block_on(relay_end_to_end())
                    })
                    .unwrap()
                    .join()
                    .unwrap();
            },
        );
        // Everything the run wrote is in the scratch folder: the sign-in and the device key and id.
        let written: Vec<String> = std::fs::read_dir(scratch.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        println!("scratch folder: {written:?}");
        assert!(
            written.iter().any(|name| name.contains("device_key")),
            "{written:?}"
        );
    }

    async fn relay_end_to_end() {
        // 1. A 1601C for the account's approved TIN: sent, with a dry-run receipt.
        let profile = test_profile();
        let (db, queued, summary) = stored_queued_1601c(&profile);
        process_queued_1601c_with_transport(
            &summary,
            &profile,
            db.clone(),
            &NetworkSubmissionTransport,
        )
        .await;
        let sent = db
            .lock()
            .unwrap()
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        assert_eq!(
            sent.status,
            FilingStatus::Submitted,
            "{:?}",
            sent.submission_error
        );
        assert!(sent.submitted_dry_run, "the server only simulated the send");
        let claims = receipt_claims(sent.relay_receipt.as_deref().expect("a receipt"));
        println!(
            "1601C receipt: mode={} status={} form={} period={}",
            claims["mode"], claims["status"], claims["form_type"], claims["period"]
        );
        assert_eq!(claims["mode"], "dry_run");
        assert_eq!(claims["form_type"], "1601Cv2018");
        assert_eq!(poll_jobs(&db), 0, "no BIR email is coming for a dry run");

        // 2. Rotate the token (the server hands out a new refresh token), then a 2551Q on the other send path.
        let before = crate::cloud_account::load_stored_account()
            .unwrap()
            .unwrap();
        let rotated = tokio::task::spawn_blocking(|| {
            crate::account_session::access_token(
                &mut crate::cloud_account::BlockingHttp::new(),
                crate::cloud_account::unix_now() + 20,
                true,
            )
        })
        .await
        .unwrap()
        .expect("a forced refresh");
        let after = crate::cloud_account::load_stored_account()
            .unwrap()
            .unwrap();
        assert_ne!(before.access_token, rotated, "a new access token");
        assert_eq!(after.access_token, rotated, "saved");
        println!(
            "refresh token rotated: {}",
            before.refresh_token != after.refresh_token
        );

        let db = test_db();
        let (queued, summary) = stored_queued_2551q(&db);
        process_queued_2551q_with_transport(
            &summary,
            &reviewed_profile(),
            db.clone(),
            &NetworkSubmissionTransport,
        )
        .await;
        let sent = db
            .lock()
            .unwrap()
            .get_2551q_draft(&queued.tin, queued.taxable_year, queued.quarter)
            .unwrap()
            .unwrap();
        assert_eq!(
            sent.status,
            FilingStatus::Submitted,
            "{:?}",
            sent.last_error
        );
        assert!(sent.submitted_dry_run);
        let claims = receipt_claims(sent.relay_receipt.as_deref().expect("a receipt"));
        println!(
            "2551Q receipt: mode={} status={} form={} period={}",
            claims["mode"], claims["status"], claims["form_type"], claims["period"]
        );
        assert_eq!(claims["mode"], "dry_run");

        // 3. A TIN that is not on the account: refused, back to Draft, with the server's sentence and code.
        let mut stranger = test_profile();
        stranger.tin.segment1 = "987".into();
        stranger.tin.segment2 = "654".into();
        stranger.tin.segment3 = "321".into();
        let (db, queued, summary) = stored_queued_1601c(&stranger);
        process_queued_1601c_with_transport(
            &summary,
            &stranger,
            db.clone(),
            &NetworkSubmissionTransport,
        )
        .await;
        let refused = db
            .lock()
            .unwrap()
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        println!(
            "refusal: status={:?} reason={:?}",
            refused.status, refused.submission_error
        );
        assert_eq!(refused.status, FilingStatus::Draft);
        assert!(refused.submission_claim_token.is_none());
        let reason = refused.submission_error.unwrap_or_default();
        assert!(
            bir_submit::wire::TinRefusal::from_reason(&reason).is_some(),
            "a TIN refusal with its code: {reason}"
        );
        assert_eq!(refused.relay_receipt, None);
    }

    #[tokio::test]
    async fn generic_worker_refuses_a_form_the_registry_does_not_authorize() {
        let profile = test_profile();
        let db = test_db();
        let (queued, summary) = queued_test_form(&db);
        let transport = RecordingSubmissionTransport::new(TestTransportOutcome::Success);

        process_queued_queueable::<TestForm, _>(
            &summary,
            &profile,
            db.clone(),
            &transport,
            crate::forms::queue_authorized_form_type_id,
        )
        .await;

        assert!(transport.calls().is_empty());
        assert_eq!(
            stored_test_form(&db, &queued).lifecycle.status,
            FilingStatus::Queued
        );
    }

    #[test]
    fn generic_cancel_returns_to_draft_and_requeue_replaces_it() {
        let db = test_db();
        let (queued, _summary) = queued_test_form(&db);
        let canceled = db.lock().unwrap().cancel_queued_queueable(&queued).unwrap();
        assert_eq!(canceled.lifecycle.status, FilingStatus::Draft);
        assert_eq!(
            stored_test_form(&db, &queued).lifecycle.status,
            FilingStatus::Draft
        );

        let mut edited = canceled;
        edited.amount = 12.0;
        db.lock().unwrap().save_queueable_draft(&edited).unwrap();
        edited
            .queue(crate::filing_queue::QueueAuthSource::Gui)
            .unwrap();
        db.lock().unwrap().save_queued_queueable(&edited).unwrap();
        let requeued = stored_test_form(&db, &queued);
        assert_eq!(requeued.lifecycle.status, FilingStatus::Queued);
        assert_eq!(requeued.amount, 12.0);

        // A queued snapshot cannot be overwritten by a draft save.
        let mut stale = requeued.clone();
        stale.lifecycle.revert_to_draft();
        assert!(db.lock().unwrap().save_queueable_draft(&stale).is_err());
    }

    #[tokio::test]
    async fn process_queued_1601c_pre_store_failure_stays_unclaimed_and_can_retry() {
        let profile = test_profile();
        let queued = queued_1601c_draft(&profile);
        let expected_fingerprint = queued.queued_submission_fingerprint.clone();
        let db = Arc::new(Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        ));
        let id = db
            .lock()
            .expect("the cron database should not be poisoned")
            .save_queued_1601c_draft(&queued)
            .expect("the reviewed 1601C queue snapshot should persist");
        let summary = queued_1601c_summary(id, &queued);
        let failing_transport = RecordingSubmissionTransport::new(TestTransportOutcome::PreStore);

        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &failing_transport)
            .await;

        assert!(failing_transport.calls().is_empty());
        let failed = db
            .lock()
            .expect("the cron database should not be poisoned")
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .expect("the unclaimed 1601C lookup should succeed")
            .expect("the unclaimed 1601C snapshot should remain persisted");
        assert_eq!(failed.status, FilingStatus::Queued);
        assert_eq!(failed.queued_submission_fingerprint, expected_fingerprint);
        assert_eq!(failed.submission_attempts, 1);
        assert!(failed.submission_claim_token.is_none());
        assert!(failed.submission_claimed_at.is_none());
        assert!(
            failed
                .submission_error
                .as_deref()
                .is_some_and(|message| message.contains("no PUT attempted"))
        );
        assert!(queued_1601c_revision(&failed).is_some());

        let expected_retry_at = failed.next_retry_at.clone();
        let mut retryable = failed.clone();
        retryable.next_retry_at = Some((Utc::now() - chrono::Duration::minutes(1)).to_rfc3339());
        assert!(
            db.lock()
                .expect("the cron database should not be poisoned")
                .replace_unclaimed_queued_1601c_submission(
                    &retryable,
                    &expected_fingerprint,
                    &expected_retry_at,
                    1,
                )
                .expect("forcing the retry timestamp should CAS-succeed")
        );

        let retry_transport = RecordingSubmissionTransport::new(TestTransportOutcome::Success);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &retry_transport).await;
        assert_eq!(retry_transport.calls().len(), 1);
        let submitted = db
            .lock()
            .expect("the cron database should not be poisoned")
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .expect("the submitted 1601C lookup should succeed")
            .expect("the submitted 1601C snapshot should remain persisted");
        assert_eq!(submitted.status, FilingStatus::Submitted);
        assert!(submitted.submission_claim_token.is_none());
        assert!(submitted.submission_claimed_at.is_none());
    }

    #[test]
    fn submission_queue_uses_the_local_year_at_the_manila_boundary() {
        let utc = Utc
            .with_ymd_and_hms(2099, 12, 31, 16, 30, 0)
            .single()
            .expect("the UTC test instant must be valid");
        let manila =
            chrono::FixedOffset::east_opt(8 * 60 * 60).expect("the Manila offset must be valid");
        let local = utc.with_timezone(&manila);

        assert_eq!(utc.year(), 2099);
        assert_eq!(local.year(), 2100);
        assert_eq!(submission_queue_year_at(&local), 2100);
    }

    #[tokio::test]
    async fn expired_unstarted_1601c_claim_recovers_and_retries() {
        let profile = test_profile();
        let queued = queued_1601c_draft(&profile);
        let fingerprint = queued.queued_submission_fingerprint.clone();
        let retry = queued.next_retry_at.clone();
        let db = Arc::new(Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        ));
        let id = db
            .lock()
            .expect("the cron database should not be poisoned")
            .save_queued_1601c_draft(&queued)
            .expect("queue");
        let summary = queued_1601c_summary(id, &queued);
        let claim = db
            .lock()
            .expect("the cron database should not be poisoned")
            .claim_queued_1601c_submission(
                &queued.tin,
                queued.taxable_year,
                queued.month,
                &fingerprint,
                &retry,
                0,
            )
            .expect("claim");
        assert!(matches!(
            claim,
            crate::db::Claim1601CSubmissionResult::Claimed { .. }
        ));

        let future = Utc::now() + chrono::Duration::hours(1);
        assert!(
            db.lock()
                .expect("the cron database should not be poisoned")
                .recover_expired_unstarted_1601c_claim(
                    &queued.tin,
                    queued.taxable_year,
                    queued.month,
                    future,
                )
                .expect("recover")
        );
        let recovered = db
            .lock()
            .expect("the cron database should not be poisoned")
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        assert!(recovered.submission_claim_token.is_none());
        assert_eq!(recovered.submission_attempts, 0);
        assert!(recovered.queue_authorization.is_some());

        let transport = RecordingSubmissionTransport::new(TestTransportOutcome::Success);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &transport).await;
        assert_eq!(transport.calls().len(), 1);
        let submitted = db
            .lock()
            .expect("the cron database should not be poisoned")
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        assert_eq!(submitted.status, FilingStatus::Submitted);
    }

    #[tokio::test]
    async fn put_started_claim_does_not_recover() {
        let profile = test_profile();
        let queued = queued_1601c_draft(&profile);
        let fingerprint = queued.queued_submission_fingerprint.clone();
        let retry = queued.next_retry_at.clone();
        let db = Arc::new(Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        ));
        db.lock()
            .expect("the cron database should not be poisoned")
            .save_queued_1601c_draft(&queued)
            .unwrap();
        let claim = db
            .lock()
            .expect("the cron database should not be poisoned")
            .claim_queued_1601c_submission(
                &queued.tin,
                queued.taxable_year,
                queued.month,
                &fingerprint,
                &retry,
                0,
            )
            .unwrap();
        let crate::db::Claim1601CSubmissionResult::Claimed { token, .. } = claim else {
            panic!("expected claim");
        };
        assert!(
            db.lock()
                .expect("the cron database should not be poisoned")
                .mark_claimed_1601c_put_started(
                    &queued.tin,
                    queued.taxable_year,
                    queued.month,
                    &token,
                )
                .unwrap()
        );
        let future = Utc::now() + chrono::Duration::hours(1);
        assert!(
            !db.lock()
                .expect("the cron database should not be poisoned")
                .recover_expired_unstarted_1601c_claim(
                    &queued.tin,
                    queued.taxable_year,
                    queued.month,
                    future,
                )
                .unwrap()
        );
    }

    #[tokio::test]
    async fn mandatory_fixture_dry_run_transport_submits_without_imap() {
        let mut profile = crate::filing_queue::mandatory_lab_1601c_profile();
        profile.email_tracking_enabled = true;
        profile.imap_email = Some("codeitlikemiley@gmail.com".to_string());
        let mut queued = crate::filing_queue::mandatory_lab_1601c_draft();
        queued.transition_to_queued().unwrap();
        let db = Arc::new(Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        ));
        let id = db
            .lock()
            .expect("the cron database should not be poisoned")
            .save_queued_1601c_draft(&queued)
            .unwrap();
        let summary = queued_1601c_summary(id, &queued);
        let transport = RecordingSubmissionTransport::dry_run(TestTransportOutcome::Success);
        process_queued_1601c_with_transport(&summary, &profile, db.clone(), &transport).await;
        assert_eq!(transport.calls().len(), 1);
        assert_eq!(
            transport.calls()[0].filename,
            "00000000000000-1601Cv2018-092026#codeitlikemiley@gmail.com#.xml"
        );
        let submitted = db
            .lock()
            .expect("the cron database should not be poisoned")
            .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
            .unwrap()
            .unwrap();
        assert_eq!(submitted.status, FilingStatus::Submitted);
        assert!(submitted.submitted_dry_run);
        assert_eq!(submitted.tax_36_total_amount_payable, 0.0);
        assert!(
            db.lock()
                .expect("the cron database should not be poisoned")
                .list_jobs()
                .unwrap()
                .iter()
                .all(|job| !is_email_poll_job(job)),
            "dry-run source must skip IMAP"
        );
    }

    /// `draft` sent for real by the worker's own path (queued, claimed, Submitted now): only a Draft may use the generic
    /// save, so a sent return is made the way the worker makes one.
    fn sent_lab_1601c(db: &Database, draft: Form1601CDraft) {
        sent_lab_1601c_via(db, draft, false);
    }

    /// As `sent_lab_1601c`; `mock_relay` marks the send as one to the local mock relay, as the worker does.
    fn sent_lab_1601c_via(db: &Database, mut draft: Form1601CDraft, mock_relay: bool) {
        draft.transition_to_queued().expect("the lab return queues");
        db.save_queued_1601c_draft(&draft)
            .expect("the queued return is saved");
        let claim = db
            .claim_queued_1601c_submission(
                &draft.tin,
                draft.taxable_year,
                draft.month,
                &draft.queued_submission_fingerprint,
                &draft.next_retry_at,
                0,
            )
            .expect("the queued return is claimed");
        let crate::db::Claim1601CSubmissionResult::Claimed {
            draft: mut claimed,
            token,
        } = claim
        else {
            panic!("expected a claim");
        };
        claimed.transition_to_submitted(draft.default_submission_filename(), false);
        claimed.submitted_via_relay = mock_relay;
        db.finish_claimed_1601c_submission(&claimed, &token)
            .expect("the send is finished");
    }

    /// #504 W1: a return still waiting for the BIR's email gets its poll back when the inbox's way in changes (here
    /// after the poll a Google grant this build cannot renew had archived); a return never sent gets none, the inbox is
    /// matched whatever its case, and a second call keeps one job per return.
    #[test]
    fn a_waiting_return_is_polled_again_when_the_inbox_changes() {
        let db = Database::open_in_memory_for_tests()
            .expect("the in-memory cron database should initialize");
        let mut profile = crate::filing_queue::mandatory_lab_1601c_profile();
        profile.email_tracking_enabled = true;
        profile.imap_email = Some("filer@example.com".to_string());
        db.save_profile(profile).expect("the profile is saved");
        sent_lab_1601c(&db, crate::filing_queue::mandatory_lab_1601c_draft());
        let draft_only = crate::filing_queue::mandatory_lab_1601c_draft_for(2026, 10);
        db.save_1601c_draft(&draft_only)
            .expect("the draft is saved");
        let polls = |db: &Database| -> Vec<Job> {
            db.list_jobs()
                .expect("the jobs are listed")
                .into_iter()
                .filter(|job| is_email_poll_job(job) && job.status != "Archived")
                .collect()
        };
        assert!(polls(&db).is_empty(), "no poll before the inbox changes");

        assert_eq!(
            reschedule_email_polls_for_inbox(&db, "Filer@Example.com"),
            1
        );
        let queued = polls(&db);
        assert_eq!(queued.len(), 1, "one poll, for the sent return only");
        assert_eq!(queued[0].status, "Queued");
        assert!(
            queued[0].name.contains("1601C Sep 2026"),
            "{}",
            queued[0].name
        );
        assert_eq!(
            queued[0].command.as_deref(),
            Some("bir_poll_email filer@example.com")
        );

        reschedule_email_polls_for_inbox(&db, "filer@example.com");
        assert_eq!(polls(&db).len(), 1, "still one job per return");
    }

    /// #504 review: Reconnect brings back the poll a Google grant this build could not renew had archived.
    #[test]
    fn reconnect_brings_back_an_archived_poll() {
        let db = Database::open_in_memory_for_tests()
            .expect("the in-memory cron database should initialize");
        let mut profile = crate::filing_queue::mandatory_lab_1601c_profile();
        profile.email_tracking_enabled = true;
        profile.imap_email = Some("filer@example.com".to_string());
        let tin = profile.tin.full();
        db.save_profile(profile).expect("the profile is saved");
        sent_lab_1601c(&db, crate::filing_queue::mandatory_lab_1601c_draft());
        reschedule_email_polls_for_inbox(&db, "filer@example.com");
        let mut job = db
            .list_jobs()
            .expect("the jobs are listed")
            .into_iter()
            .find(is_email_poll_job)
            .expect("the waiting return has a poll");
        job.status = "Archived".to_string();
        db.save_job(job).expect("the poll is archived");
        let live = |db: &Database| {
            db.list_jobs()
                .expect("the jobs are listed")
                .into_iter()
                .filter(|job| is_email_poll_job(job) && job.status == "Queued")
                .count()
        };
        assert_eq!(live(&db), 0);

        persist_google_grant_and_resume_polls(
            &db,
            "filer@example.com",
            "access-new",
            "refresh-new",
            Some(&tin),
        )
        .expect("the grant is saved");
        assert_eq!(live(&db), 1, "the waiting return is polled again");
    }

    /// #504 review: a send to the local mock relay never had a poll (the worker skips it), so a change of the inbox's
    /// way in queues none for it either; a real send beside it still gets its poll.
    #[test]
    fn a_mock_relay_send_gets_no_poll_back() {
        let db = Database::open_in_memory_for_tests()
            .expect("the in-memory cron database should initialize");
        let mut profile = crate::filing_queue::mandatory_lab_1601c_profile();
        profile.email_tracking_enabled = true;
        profile.imap_email = Some("filer@example.com".to_string());
        db.save_profile(profile).expect("the profile is saved");
        sent_lab_1601c_via(&db, crate::filing_queue::mandatory_lab_1601c_draft(), true);
        let tin = crate::filing_queue::mandatory_lab_1601c_draft().tin;
        assert!(
            db.returns_awaiting_bir_email(&tin)
                .expect("the waiting returns are listed")
                .is_empty(),
            "a mock relay send is not waiting for the BIR's email"
        );
        sent_lab_1601c(
            &db,
            crate::filing_queue::mandatory_lab_1601c_draft_for(2026, 10),
        );
        assert_eq!(
            db.returns_awaiting_bir_email(&tin)
                .expect("the waiting returns are listed")
                .len(),
            1,
            "the real send is"
        );
    }

    /// With tracking off the inbox is not watched, so nothing is queued however it changes.
    #[test]
    fn an_inbox_with_tracking_off_gets_no_poll_back() {
        let db = Database::open_in_memory_for_tests()
            .expect("the in-memory cron database should initialize");
        let mut profile = crate::filing_queue::mandatory_lab_1601c_profile();
        profile.email_tracking_enabled = false;
        profile.imap_email = Some("filer@example.com".to_string());
        db.save_profile(profile).expect("the profile is saved");
        sent_lab_1601c(&db, crate::filing_queue::mandatory_lab_1601c_draft());

        assert_eq!(
            reschedule_email_polls_for_inbox(&db, "filer@example.com"),
            1
        );
        assert!(
            db.list_jobs()
                .expect("the jobs are listed")
                .iter()
                .all(|job| !is_email_poll_job(job))
        );
    }

    #[test]
    fn schedule_email_poll_uses_period_dashed_tin_and_optional_email() {
        let mut profile = crate::filing_queue::mandatory_lab_1601c_profile();
        profile.email_tracking_enabled = true;
        profile.imap_email = Some("codeitlikemiley@gmail.com".to_string());
        let draft = crate::filing_queue::mandatory_lab_1601c_draft();
        let db = Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        );
        let guard = db.lock().expect("the cron database should not be poisoned");
        schedule_email_poll(
            &profile,
            "1601C",
            &draft.tin,
            draft.taxable_year,
            Some(draft.month),
            None,
            &guard,
        );
        schedule_email_poll(
            &profile,
            "1601C",
            &draft.tin,
            draft.taxable_year,
            Some(draft.month),
            None,
            &guard,
        );
        let jobs = guard
            .list_jobs()
            .expect("the email-poll job lookup should succeed");
        assert_eq!(
            jobs.len(),
            1,
            "identical identity must not insert a second poll job"
        );
        assert_eq!(
            jobs[0].name,
            "Waiting for 1601C Sep 2026 confirmation for 000-000-000-00000 (codeitlikemiley@gmail.com)"
        );
        assert_eq!(
            jobs[0].command.as_deref(),
            Some("bir_poll_email codeitlikemiley@gmail.com")
        );
        assert_eq!(jobs[0].status, "Queued");
        assert_eq!(jobs[0].job_type, "System");
    }

    #[test]
    fn schedule_email_poll_creates_distinct_jobs_per_period() {
        let mut profile = crate::filing_queue::mandatory_lab_1601c_profile();
        profile.email_tracking_enabled = true;
        let september = crate::filing_queue::mandatory_lab_1601c_draft();
        let october = crate::filing_queue::mandatory_lab_1601c_october_2026_draft();
        let db = Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        );
        let guard = db.lock().expect("the cron database should not be poisoned");
        schedule_email_poll(
            &profile,
            "1601C",
            &september.tin,
            september.taxable_year,
            Some(september.month),
            None,
            &guard,
        );
        schedule_email_poll(
            &profile,
            "1601C",
            &october.tin,
            october.taxable_year,
            Some(october.month),
            None,
            &guard,
        );
        let mut names: Vec<String> = guard
            .list_jobs()
            .expect("the email-poll job lookup should succeed")
            .into_iter()
            .map(|job| job.name)
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "Waiting for 1601C Oct 2026 confirmation for 000-000-000-00000 (codeitlikemiley@gmail.com)"
                    .to_string(),
                "Waiting for 1601C Sep 2026 confirmation for 000-000-000-00000 (codeitlikemiley@gmail.com)"
                    .to_string(),
            ]
        );
    }

    #[test]
    fn schedule_email_poll_omits_email_parenthetical_when_unknown() {
        let mut profile = crate::filing_queue::mandatory_lab_1601c_profile();
        profile.email_tracking_enabled = true;
        profile.email.clear();
        profile.imap_email = None;
        let draft = crate::filing_queue::mandatory_lab_1601c_draft();
        let db = Mutex::new(
            Database::open_in_memory_for_tests()
                .expect("the in-memory cron database should initialize"),
        );
        let guard = db.lock().expect("the cron database should not be poisoned");
        schedule_email_poll(
            &profile,
            "1601C",
            &draft.tin,
            draft.taxable_year,
            Some(draft.month),
            None,
            &guard,
        );
        let jobs = guard
            .list_jobs()
            .expect("the email-poll job lookup should succeed");
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            jobs[0].name,
            "Waiting for 1601C Sep 2026 confirmation for 000-000-000-00000"
        );
        assert!(
            !jobs[0].name.contains("()"),
            "unknown email must not invent a parenthetical"
        );
        assert_eq!(jobs[0].command.as_deref(), Some("bir_poll_email "));
    }

    // ===== A relay send the app did not see end: noted, then settled with Buwiz =====

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum SendPath {
        Monthly1601C,
        Quarterly2551Q,
        Generic,
    }

    const SEND_PATHS: [SendPath; 3] = [
        SendPath::Monthly1601C,
        SendPath::Quarterly2551Q,
        SendPath::Generic,
    ];

    /// One queued return on one send path, and the worker run on it.
    struct Queue {
        path: SendPath,
        db: Arc<Mutex<Database>>,
        summary: FormDraftSummary,
        profile: TaxpayerProfile,
        tin: String,
        year: u16,
        period: u8,
        form_code: &'static str,
    }

    /// What these tests read off the stored return.
    #[derive(Debug)]
    struct Stored {
        status: FilingStatus,
        claimed: bool,
        token: Option<String>,
        put_started: bool,
        pending: Option<bir_submit::PendingSend>,
        attempts: u32,
        error: Option<String>,
        next_retry_at: Option<chrono::DateTime<Utc>>,
        relay_receipt: Option<String>,
        dry_run: bool,
        fingerprint: Option<String>,
    }

    impl Queue {
        fn new(path: SendPath) -> Self {
            match path {
                SendPath::Monthly1601C => {
                    let profile = test_profile();
                    let (db, queued, summary) = stored_queued_1601c(&profile);
                    Self {
                        path,
                        db,
                        summary,
                        profile,
                        tin: queued.tin.clone(),
                        year: queued.taxable_year,
                        period: queued.month,
                        form_code: "1601C",
                    }
                }
                SendPath::Quarterly2551Q => {
                    let db = test_db();
                    let (queued, summary) = stored_queued_2551q(&db);
                    Self {
                        path,
                        db,
                        summary,
                        profile: reviewed_profile(),
                        tin: queued.tin.clone(),
                        year: queued.taxable_year,
                        period: queued.quarter,
                        form_code: "2551Q",
                    }
                }
                SendPath::Generic => {
                    let db = test_db();
                    let (queued, summary) = queued_test_form(&db);
                    Self {
                        path,
                        db,
                        summary,
                        profile: test_profile(),
                        tin: queued.tin.clone(),
                        year: queued.taxable_year,
                        period: queued.month,
                        form_code: TestForm::CODE,
                    }
                }
            }
        }

        async fn run(&self, transport: &RecordingSubmissionTransport) {
            let db = self.db.clone();
            match self.path {
                SendPath::Monthly1601C => {
                    process_queued_1601c_with_transport(&self.summary, &self.profile, db, transport)
                        .await
                }
                SendPath::Quarterly2551Q => {
                    process_queued_2551q_with_transport(&self.summary, &self.profile, db, transport)
                        .await
                }
                SendPath::Generic => {
                    process_queued_queueable::<TestForm, _>(
                        &self.summary,
                        &self.profile,
                        db,
                        transport,
                        allow_test_form,
                    )
                    .await
                }
            }
        }

        /// The app is stopped during a Buwiz relay send, after both notes.
        async fn killed_during_the_send(&self) {
            let transport = RecordingSubmissionTransport::buwiz(TestTransportOutcome::BuwizKilled);
            let ran =
                tokio::time::timeout(std::time::Duration::from_millis(200), self.run(&transport))
                    .await;
            assert!(ran.is_err(), "the send never ended");
            assert_eq!(*transport.noted.lock().unwrap(), [Ok(()), Ok(())]);
        }

        fn read(&self) -> Stored {
            let db = self.db.lock().unwrap();
            let pending = |raw: &Option<String>| {
                raw.as_deref()
                    .map(|raw| serde_json::from_str::<bir_submit::PendingSend>(raw).unwrap())
            };
            match self.path {
                SendPath::Monthly1601C => {
                    let d = db
                        .get_1601c_draft(&self.tin, self.year, self.period)
                        .unwrap()
                        .unwrap();
                    Stored {
                        status: d.status.clone(),
                        claimed: d.submission_claim_token.is_some(),
                        token: d.submission_claim_token.clone(),
                        put_started: d.submission_put_started_at.is_some(),
                        pending: pending(&d.relay_pending),
                        attempts: d.submission_attempts,
                        error: d.submission_error.clone(),
                        next_retry_at: retry_time(d.next_retry_at.as_deref()),
                        relay_receipt: d.relay_receipt.clone(),
                        dry_run: d.submitted_dry_run,
                        fingerprint: d.queued_submission_fingerprint.clone(),
                    }
                }
                SendPath::Quarterly2551Q => {
                    let d = db
                        .get_2551q_draft(&self.tin, self.year, self.period)
                        .unwrap()
                        .unwrap();
                    Stored {
                        status: d.status.clone(),
                        claimed: d.submission_claim_token.is_some(),
                        token: d.submission_claim_token.clone(),
                        put_started: d.submission_put_started_at.is_some(),
                        pending: pending(&d.relay_pending),
                        attempts: d.submission_attempts,
                        error: d.last_error.clone(),
                        next_retry_at: retry_time(d.next_retry_at.as_deref()),
                        relay_receipt: d.relay_receipt.clone(),
                        dry_run: d.submitted_dry_run,
                        fingerprint: d.queued_submission_fingerprint.clone(),
                    }
                }
                SendPath::Generic => {
                    let d = db
                        .get_queueable_draft::<TestForm>(
                            &self.tin,
                            self.year,
                            i64::from(self.period),
                        )
                        .unwrap()
                        .unwrap();
                    let l = &d.lifecycle;
                    Stored {
                        status: l.status.clone(),
                        claimed: !l.is_unclaimed(),
                        token: l.submission_claim_token.clone(),
                        put_started: l.submission_put_started_at.is_some(),
                        pending: pending(&l.relay_pending),
                        attempts: l.submission_attempts,
                        error: l.submission_error.clone(),
                        next_retry_at: retry_time(l.next_retry_at.as_deref()),
                        relay_receipt: l.relay_receipt.clone(),
                        dry_run: l.submitted_dry_run,
                        fingerprint: l.queued_submission_fingerprint.clone(),
                    }
                }
            }
        }

        /// Write `pending` on the return's claim as `token` holds it.
        fn note(&self, token: &str, pending: Option<&str>) -> bool {
            let db = self.db.lock().unwrap();
            match self.path {
                SendPath::Monthly1601C => db.note_claimed_1601c_relay_pending(
                    &self.tin,
                    self.year,
                    self.period,
                    token,
                    pending,
                ),
                SendPath::Quarterly2551Q => db.note_claimed_2551q_relay_pending(
                    &self.tin,
                    self.year,
                    self.period,
                    token,
                    pending,
                ),
                SendPath::Generic => db.note_claimed_queueable_relay_pending::<TestForm>(
                    &self.tin,
                    self.year,
                    i64::from(self.period),
                    token,
                    pending,
                ),
            }
            .unwrap()
        }

        /// The noted send, as if it had been noted `hours` ago.
        fn age_pending(&self, hours: i64) {
            let stored = self.read();
            let pending = bir_submit::PendingSend {
                noted_at: Utc::now() - chrono::Duration::hours(hours),
                ..stored.pending.unwrap()
            };
            let json = serde_json::to_string(&pending).unwrap();
            assert!(self.note(&stored.token.unwrap(), Some(&json)));
        }
    }

    /// After the app was stopped during the send: the return as the next worker leaves it once Buwiz said
    /// `settles`, how many times it was asked, and whether any run (that one, and one more whose transport would
    /// send) sent the return again.
    struct Reconciled {
        path: SendPath,
        stored: Stored,
        asked: usize,
        sent_again: usize,
        poll_jobs: usize,
    }

    async fn reconciled(
        path: SendPath,
        settles: fn(&bir_submit::PendingSend) -> crate::relay::Settlement,
        aged_hours: i64,
    ) -> Reconciled {
        let queue = Queue::new(path);
        queue.killed_during_the_send().await;
        let killed = queue.read();
        assert!(killed.claimed && killed.put_started, "{path:?}");
        let noted = killed.pending.as_ref().expect("the send was noted");
        assert!(noted.opening, "{path:?}: about to open toward the BIR");
        if aged_hours > 0 {
            queue.age_pending(aged_hours);
        }
        let next = RecordingSubmissionTransport::settling(TestTransportOutcome::BuwizSent, settles);
        queue.run(&next).await;
        let stored = queue.read();
        let poll_jobs = poll_jobs(&queue.db);
        let again = RecordingSubmissionTransport::buwiz(TestTransportOutcome::BuwizSent);
        queue.run(&again).await;
        Reconciled {
            path,
            stored,
            asked: next.asked(),
            sent_again: next.calls().len() + again.calls().len(),
            poll_jobs,
        }
    }

    async fn reconciled_on_every_path(
        settles: fn(&bir_submit::PendingSend) -> crate::relay::Settlement,
        aged_hours: i64,
    ) -> Vec<Reconciled> {
        let mut all = Vec::new();
        for path in SEND_PATHS {
            all.push(reconciled(path, settles, aged_hours).await);
        }
        all
    }

    fn sent_receipt(_: &bir_submit::PendingSend) -> crate::relay::Settlement {
        crate::relay::Settlement::Sent(crate::transport::StoreReport {
            relay_receipt: Some(BUWIZ_RECEIPT.into()),
            simulated: false,
        })
    }

    #[tokio::test]
    async fn a_send_buwiz_says_went_is_finished_with_its_receipt_and_never_sent_again() {
        for r in reconciled_on_every_path(sent_receipt, 0).await {
            let path = r.path;
            assert_eq!(r.asked, 1, "{path:?}");
            assert_eq!(r.stored.status, FilingStatus::Submitted, "{path:?}");
            assert_eq!(
                r.stored.relay_receipt.as_deref(),
                Some(BUWIZ_RECEIPT),
                "{path:?}"
            );
            assert!(!r.stored.dry_run, "{path:?}: a real send");
            assert!(!r.stored.claimed, "{path:?}");
            assert!(r.stored.pending.is_none(), "{path:?}");
            assert_eq!(r.stored.error, None, "{path:?}");
            assert_eq!(r.sent_again, 0, "{path:?}");
        }
    }

    #[tokio::test]
    async fn a_receipt_buwiz_only_simulated_finishes_as_a_dry_run() {
        fn simulated(_: &bir_submit::PendingSend) -> crate::relay::Settlement {
            crate::relay::Settlement::Sent(crate::transport::StoreReport {
                relay_receipt: Some(BUWIZ_RECEIPT.into()),
                simulated: true,
            })
        }
        for r in reconciled_on_every_path(simulated, 0).await {
            assert_eq!(r.stored.status, FilingStatus::Submitted, "{:?}", r.path);
            assert!(r.stored.dry_run, "{:?}", r.path);
            assert_eq!(r.poll_jobs, 0, "{:?}: no BIR email is coming", r.path);
        }
    }

    /// The settlement the real relay gives for the client's answer.
    fn failed(error: bir_submit::SendError) -> crate::relay::Settlement {
        crate::relay::settlement(bir_submit::Settled::Failed(error))
    }

    #[tokio::test]
    async fn a_refusal_buwiz_reports_goes_back_to_draft_with_its_sentence() {
        fn refused(_: &bir_submit::PendingSend) -> crate::relay::Settlement {
            failed(bir_submit::SendError::NotSent(BUWIZ_REFUSAL.into()))
        }
        for r in reconciled_on_every_path(refused, 0).await {
            assert_eq!(r.stored.status, FilingStatus::Draft, "{:?}", r.path);
            assert_eq!(
                r.stored.error.as_deref(),
                Some(BUWIZ_REFUSAL),
                "{:?}",
                r.path
            );
            assert!(
                !r.stored.claimed && r.stored.pending.is_none(),
                "{:?}",
                r.path
            );
            assert_eq!(r.sent_again, 0, "{:?}", r.path);
        }
    }

    #[tokio::test]
    async fn a_counted_failure_buwiz_reports_waits_on_the_ladder() {
        fn counted(_: &bir_submit::PendingSend) -> crate::relay::Settlement {
            failed(bir_submit::SendError::Counted {
                reason: "BIR's upload server did not answer".into(),
                blame: bir_submit::Blame::Upload,
            })
        }
        for r in reconciled_on_every_path(counted, 0).await {
            assert_eq!(r.stored.status, FilingStatus::Queued, "{:?}", r.path);
            assert!(
                !r.stored.claimed && r.stored.pending.is_none(),
                "{:?}",
                r.path
            );
            assert_eq!(r.stored.attempts, 1, "{:?}", r.path);
            assert!(
                (4..=5).contains(&minutes_from_now(r.stored.next_retry_at)),
                "{:?}",
                r.path
            );
            assert_eq!(r.sent_again, 0, "{:?}: not before its time", r.path);
        }
    }

    /// `failed` or `expired` before any connection toward the BIR opened: nothing touched the BIR, not a try. The
    /// return is held, and once its time comes it is sent again.
    #[tokio::test]
    async fn a_send_that_never_reached_the_bir_is_held_and_goes_again_later() {
        fn not_a_try(_: &bir_submit::PendingSend) -> crate::relay::Settlement {
            failed(bir_submit::SendError::NotATry {
                reason: "the connection ended before sending".into(),
                wait_until: None,
            })
        }
        for path in SEND_PATHS {
            let r = reconciled(path, not_a_try, 0).await;
            assert_eq!(r.stored.status, FilingStatus::Queued, "{path:?}");
            assert!(!r.stored.claimed && r.stored.pending.is_none(), "{path:?}");
            assert_eq!(r.stored.attempts, 0, "{path:?}: nothing is counted");
            assert!(
                r.stored.fingerprint.is_some(),
                "{path:?}: still the reviewed return"
            );
            assert!(
                (4..=5).contains(&minutes_from_now(r.stored.next_retry_at)),
                "{path:?}"
            );
            assert_eq!(r.sent_again, 0, "{path:?}: not before its time");
        }
        // Its time come, the worker sends it.
        for path in SEND_PATHS {
            let queue = Queue::new(path);
            queue.killed_during_the_send().await;
            queue
                .run(&RecordingSubmissionTransport::settling(
                    TestTransportOutcome::BuwizSent,
                    not_a_try,
                ))
                .await;
            make_retry_due(&queue.db, queue.form_code, &queue.tin);
            let later = RecordingSubmissionTransport::buwiz(TestTransportOutcome::BuwizSent);
            queue.run(&later).await;
            assert_eq!(later.calls().len(), 1, "{path:?}");
            assert_eq!(queue.read().status, FilingStatus::Submitted, "{path:?}");
        }
    }

    #[tokio::test]
    async fn a_hold_buwiz_reports_waits_as_the_server_says() {
        fn held(_: &bir_submit::PendingSend) -> crate::relay::Settlement {
            crate::relay::Settlement::Failed(crate::transport::TransportError::RelayHeld {
                reason: BUWIZ_HOLD.into(),
                wait_until: Some(buwiz_hold_until()),
            })
        }
        for r in reconciled_on_every_path(held, 0).await {
            assert_eq!(r.stored.status, FilingStatus::Queued, "{:?}", r.path);
            assert!(
                !r.stored.claimed && r.stored.pending.is_none(),
                "{:?}",
                r.path
            );
            assert_eq!(r.stored.attempts, 0, "{:?}", r.path);
            assert_eq!(r.stored.error.as_deref(), Some(BUWIZ_HOLD), "{:?}", r.path);
            assert_eq!(r.sent_again, 0, "{:?}", r.path);
        }
    }

    /// Maybe sent (a receipt that does not verify, `unknown`, `expired` after opening): the claim stays, the
    /// record goes, so the person answers; nothing sends it again.
    #[tokio::test]
    async fn an_unknown_outcome_buwiz_reports_keeps_the_claim_and_asks_the_person() {
        fn unknown(_: &bir_submit::PendingSend) -> crate::relay::Settlement {
            failed(bir_submit::SendError::MaybeSent(
                "the server reports the return sent, but its receipt does not check out".into(),
            ))
        }
        for r in reconciled_on_every_path(unknown, 0).await {
            assert_eq!(r.stored.status, FilingStatus::Queued, "{:?}", r.path);
            assert!(r.stored.claimed && r.stored.put_started, "{:?}", r.path);
            assert!(
                r.stored.pending.is_none(),
                "{:?}: the buttons appear",
                r.path
            );
            assert_eq!(
                r.stored.relay_receipt, None,
                "{:?}: never sent without a receipt",
                r.path
            );
            let error = r.stored.error.clone().unwrap_or_default();
            assert!(error.starts_with(OUTCOME_UNKNOWN), "{:?}: {error}", r.path);
            assert_eq!(r.asked, 1, "{:?}", r.path);
            assert_eq!(r.sent_again, 0, "{:?}", r.path);
        }
    }

    #[tokio::test]
    async fn not_yet_leaves_the_return_as_it_is_until_a_day_has_passed() {
        fn not_yet(_: &bir_submit::PendingSend) -> crate::relay::Settlement {
            crate::relay::Settlement::NotYet
        }
        for r in reconciled_on_every_path(not_yet, 0).await {
            assert_eq!(r.stored.status, FilingStatus::Queued, "{:?}", r.path);
            assert!(r.stored.claimed, "{:?}", r.path);
            assert!(
                r.stored.pending.is_some(),
                "{:?}: asked again later",
                r.path
            );
            assert_eq!(r.asked, 1, "{:?}", r.path);
            assert_eq!(r.sent_again, 0, "{:?}", r.path);
        }
        for r in reconciled_on_every_path(not_yet, 25).await {
            assert!(r.stored.claimed, "{:?}", r.path);
            assert!(
                r.stored.pending.is_none(),
                "{:?}: the person is asked",
                r.path
            );
            assert_eq!(r.sent_again, 0, "{:?}", r.path);
        }
    }

    #[tokio::test]
    async fn a_submission_buwiz_does_not_know_is_left_to_the_person() {
        fn gone(_: &bir_submit::PendingSend) -> crate::relay::Settlement {
            crate::relay::Settlement::Gone
        }
        for r in reconciled_on_every_path(gone, 0).await {
            assert_eq!(r.stored.status, FilingStatus::Queued, "{:?}", r.path);
            assert!(
                r.stored.claimed && r.stored.pending.is_none(),
                "{:?}",
                r.path
            );
            assert_eq!(r.stored.relay_receipt, None, "{:?}", r.path);
            assert_eq!(r.sent_again, 0, "{:?}", r.path);
        }
    }

    /// Once Buwiz said it went, nothing it says later moves the return.
    #[tokio::test]
    async fn a_return_buwiz_said_was_sent_is_never_released_for_another_try() {
        fn counted(_: &bir_submit::PendingSend) -> crate::relay::Settlement {
            crate::relay::Settlement::Failed(crate::transport::TransportError::RelayFailed(
                "x".into(),
            ))
        }
        for path in SEND_PATHS {
            let queue = Queue::new(path);
            queue.killed_during_the_send().await;
            queue
                .run(&RecordingSubmissionTransport::settling(
                    TestTransportOutcome::BuwizSent,
                    sent_receipt,
                ))
                .await;
            let later =
                RecordingSubmissionTransport::settling(TestTransportOutcome::BuwizSent, counted);
            queue.run(&later).await;
            assert_eq!(later.asked(), 0, "{path:?}");
            assert!(later.calls().is_empty(), "{path:?}");
            let stored = queue.read();
            assert_eq!(stored.status, FilingStatus::Submitted, "{path:?}");
            assert_eq!(stored.attempts, 0, "{path:?}");
        }
    }

    /// In this process the worker notes every Buwiz send and drops the record when the claim goes; an unknown
    /// outcome keeps it, for the next tick to ask Buwiz. No other send writes one.
    #[tokio::test]
    async fn the_record_lives_exactly_as_long_as_the_claim_of_a_buwiz_send() {
        for outcome in [
            TestTransportOutcome::BuwizSent,
            TestTransportOutcome::BuwizRefused,
            TestTransportOutcome::BuwizFailed,
            TestTransportOutcome::BuwizHeld,
        ] {
            for path in SEND_PATHS {
                let queue = Queue::new(path);
                let transport = RecordingSubmissionTransport::buwiz(outcome);
                queue.run(&transport).await;
                assert_eq!(
                    *transport.noted.lock().unwrap(),
                    [Ok(()), Ok(())],
                    "{path:?}"
                );
                let stored = queue.read();
                assert!(!stored.claimed, "{outcome:?} {path:?}");
                assert!(stored.pending.is_none(), "{outcome:?} {path:?}");
            }
        }
        for path in SEND_PATHS {
            let queue = Queue::new(path);
            queue
                .run(&RecordingSubmissionTransport::buwiz(
                    TestTransportOutcome::BuwizUnknown,
                ))
                .await;
            let stored = queue.read();
            assert!(stored.claimed, "{path:?}");
            let pending = stored.pending.expect("kept for Buwiz to answer");
            assert!(pending.opening, "{path:?}");
            // The next tick asks.
            let next = RecordingSubmissionTransport::settling(
                TestTransportOutcome::BuwizSent,
                sent_receipt,
            );
            queue.run(&next).await;
            assert_eq!(next.asked(), 1, "{path:?}");
            assert!(next.calls().is_empty(), "{path:?}");
            assert_eq!(queue.read().status, FilingStatus::Submitted, "{path:?}");
        }
        for transport in [
            RecordingSubmissionTransport::new(TestTransportOutcome::Success),
            RecordingSubmissionTransport::dry_run(TestTransportOutcome::Success),
            RecordingSubmissionTransport::relay(TestTransportOutcome::UnknownIo),
        ] {
            for path in SEND_PATHS {
                let queue = Queue::new(path);
                queue.run(&transport).await;
                assert!(queue.read().pending.is_none(), "{path:?}");
                assert!(
                    !stored_json(&queue.db, queue.form_code, &queue.tin).contains("relay_pending"),
                    "{path:?}"
                );
            }
            assert!(transport.noted.lock().unwrap().is_empty());
        }
    }

    /// Only the worker holding the claim writes the record, and a refused note stops the send before anything
    /// leaves this computer (the client's side of it is tested in bir-submit).
    #[tokio::test]
    async fn only_the_claim_holder_notes_a_send() {
        for path in SEND_PATHS {
            let queue = Queue::new(path);
            assert!(
                !queue.note("not-a-claim", Some("{}")),
                "{path:?}: unclaimed"
            );
            queue.killed_during_the_send().await;
            assert!(!queue.note("someone-else", None), "{path:?}");
            assert!(queue.read().pending.is_some(), "{path:?}");
        }
    }

    #[test]
    fn the_step_follows_what_buwiz_said_and_waits_a_day_at_most() {
        use crate::relay::Settlement;
        let now = Utc::now();
        let noted = |hours: i64| {
            serde_json::to_string(&test_pending(
                b"<xml/>",
                true,
                now - chrono::Duration::hours(hours),
            ))
            .unwrap()
        };
        assert!(matches!(
            reconcile_step(Settlement::NotYet, &noted(23), now),
            Reconcile::Keep
        ));
        assert!(matches!(
            reconcile_step(Settlement::NotYet, &noted(24), now),
            Reconcile::Clear
        ));
        assert!(matches!(
            reconcile_step(Settlement::NotYet, "not a record", now),
            Reconcile::Clear
        ));
        assert!(matches!(
            reconcile_step(Settlement::Gone, &noted(0), now),
            Reconcile::Clear
        ));
        assert!(matches!(
            reconcile_step(Settlement::Sent(Default::default()), &noted(30), now),
            Reconcile::Finish(_)
        ));
        let xml = "<xml/>".to_owned();
        assert_eq!(
            sent_payload(&noted(0), [Some("<other/>".into()), Some(xml.clone())]),
            Some(xml)
        );
        assert_eq!(
            sent_payload(&noted(0), [Some("<other/>".into()), None]),
            None
        );
    }

    // ---------- the worker's lifecycle (one engine after another in one process) ----------

    /// A send that never ends on its own: it says when it has started, then waits forever (the relay that does not
    /// answer). How many sends started is counted.
    struct HangingTransport {
        started: Mutex<std::sync::mpsc::Sender<()>>,
        sends: std::sync::atomic::AtomicUsize,
    }

    impl SubmissionTransport for HangingTransport {
        type Session = String;

        fn session_source(&self, _session: &Self::Session) -> crate::transport::SftpEndpointSource {
            crate::transport::SftpEndpointSource::Relay
        }

        fn open_session<'a>(
            &'a self,
            form_type: &'a str,
            _tin: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Self::Session, crate::transport::TransportError>>
                    + Send
                    + 'a,
            >,
        > {
            let form_type = form_type.to_string();
            Box::pin(async move { Ok(form_type) })
        }

        fn store_session<'a>(
            &'a self,
            _session: Self::Session,
            _filename: &'a str,
            _payload: &'a [u8],
            _xml: &'a [u8],
            _note: bir_submit::Note<'a>,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            crate::transport::StoreReport,
                            crate::transport::TransportError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            self.sends.fetch_add(1, Ordering::SeqCst);
            let _ = self.started.lock().unwrap().send(());
            Box::pin(std::future::pending())
        }

        /// Not a Buwiz send, so nothing is noted and this is never asked; it would not answer either.
        fn settle_pending<'a>(
            &'a self,
            _pending: &'a str,
        ) -> Pin<Box<dyn Future<Output = crate::relay::Settlement> + Send + 'a>> {
            Box::pin(std::future::pending())
        }
    }

    /// An engine ends while its worker is sending: the worker's runtime is shut down, which drops the send where
    /// it waits. Nothing of it is left: its job key is gone and no clone of the database handle survives it. The
    /// return keeps its claim and "upload started", so the next worker finds it claimed and does not send it again
    /// (what a desktop relaunch after a quit mid-send finds).
    #[test]
    fn a_send_dropped_mid_flight_keeps_its_claim_and_frees_its_job_key() {
        const KEY: &str = "form:1601C:lifecycle-test:dropped-mid-send";
        let profile = test_profile();
        let queued = queued_1601c_draft(&profile);
        let db = Arc::new(Mutex::new(Database::open_in_memory_for_tests().unwrap()));
        let id = db.lock().unwrap().save_queued_1601c_draft(&queued).unwrap();
        let summary = queued_1601c_summary(id, &queued);
        let (started, sending) = std::sync::mpsc::channel();
        let transport = Arc::new(HangingTransport {
            started: Mutex::new(started),
            sends: std::sync::atomic::AtomicUsize::new(0),
        });

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        assert!(get_active_jobs().lock().unwrap().insert(KEY.to_string()));
        let cleanup = JobCleanup {
            key: KEY.to_string(),
            active_jobs: get_active_jobs(),
        };
        let (job_db, job_transport, job_profile) = (db.clone(), transport.clone(), profile.clone());
        runtime.block_on(async move {
            spawn_job(cleanup, async move {
                process_queued_1601c_with_transport(
                    &summary,
                    &job_profile,
                    job_db,
                    &*job_transport,
                )
                .await;
            });
        });
        sending
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("the send started");
        assert!(get_active_jobs().lock().unwrap().contains(KEY), "sending");

        let ended = std::time::Instant::now();
        runtime.shutdown_timeout(std::time::Duration::from_secs(1));
        // Room for a loaded machine: the bound that matters is the worker's 5 s.
        assert!(
            ended.elapsed() < std::time::Duration::from_secs(5),
            "dropped, not waited for"
        );
        assert!(
            !get_active_jobs().lock().unwrap().contains(KEY),
            "the key went with the task"
        );
        assert_eq!(
            Arc::strong_count(&db),
            1,
            "no task kept the database handle"
        );

        let stored = || {
            db.lock()
                .unwrap()
                .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
                .unwrap()
                .unwrap()
        };
        let claimed = stored();
        assert_eq!(claimed.status, FilingStatus::Queued);
        assert!(claimed.submission_claim_token.is_some(), "the claim stays");
        assert!(
            claimed.submission_put_started_at.is_some(),
            "upload started stays"
        );
        assert!(claimed.submitted_at.is_none());

        // The next engine's worker: the claim is found, nothing is sent again.
        let next = RecordingSubmissionTransport::new(TestTransportOutcome::Success);
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(process_queued_1601c_with_transport(
                &queued_1601c_summary(id, &queued),
                &profile,
                db.clone(),
                &next,
            ));
        assert!(
            next.calls().is_empty(),
            "a claimed return is not sent again"
        );
        assert_eq!(transport.sends.load(Ordering::SeqCst), 1);
        let after = stored();
        assert_eq!(after.status, FilingStatus::Queued);
        assert_eq!(after.submission_claim_token, claimed.submission_claim_token);
    }

    /// A job the runtime drops before it ever ran still gives its key back: the guard is part of the task's future,
    /// not made inside it on the first poll.
    #[test]
    fn a_job_dropped_before_it_ever_ran_frees_its_key() {
        const KEY: &str = "form:1601C:lifecycle-test:never-polled";
        let ran = Arc::new(AtomicBool::new(false));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(get_active_jobs().lock().unwrap().insert(KEY.to_string()));
        let cleanup = JobCleanup {
            key: KEY.to_string(),
            active_jobs: get_active_jobs(),
        };
        let job_ran = ran.clone();
        runtime.block_on(async move {
            spawn_job(cleanup, async move {
                job_ran.store(true, Ordering::SeqCst);
                std::future::pending::<()>().await;
            });
        });
        assert!(
            get_active_jobs().lock().unwrap().contains(KEY),
            "spawned, not yet run"
        );
        drop(runtime);
        assert!(
            !ran.load(Ordering::SeqCst),
            "the runtime dropped the job before it ran"
        );
        assert!(
            !get_active_jobs().lock().unwrap().contains(KEY),
            "its key went with it"
        );
    }

    /// One loop after another in one process, as one engine after another starts its worker: a loop's own stop
    /// ends it and nothing else, the next is not born stopped, and each puts its own wake channel in and takes it
    /// out. `request_shutdown` (the `bir` command line's quit) stops the loop running then, not one started after it.
    /// In a child process of its own: the wake channel and the shutdown generation are the process's, and no other
    /// test may share them while this one reads them.
    #[test]
    fn each_loop_stops_on_its_own_signal_and_the_next_is_not_born_stopped() {
        const CHILD: &str = "BIR_CORE_CRON_LOOP_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "background_cron::tests::each_loop_stops_on_its_own_signal_and_the_next_is_not_born_stopped",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env("EBIR_TEST_ENV", "1")
                .env("BIR_SFTP_DRY_RUN", "1")
                .env_remove("BIR_SFTP_LIVE")
                .env_remove("BIR_RELAY_URL")
                .status()
                .unwrap();
            assert!(status.success(), "the child process failed");
            return;
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let db = Arc::new(Mutex::new(Database::open_in_memory_for_tests().unwrap()));
        let bound = std::time::Duration::from_secs(5);
        runtime.block_on(async {
            let until_woken = || async {
                let deadline = tokio::time::Instant::now() + bound;
                while wake_with_status() == WakeDispatchStatus::WorkerNotRunning {
                    assert!(
                        tokio::time::Instant::now() < deadline,
                        "the loop never started"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            };
            for engine in 1..=3 {
                let stop = CronStop::new();
                let job = tokio::spawn(run_cron_jobs(db.clone(), stop.clone()));
                until_woken().await;
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                assert!(!job.is_finished(), "engine {engine}: the loop runs");
                stop.stop();
                tokio::time::timeout(bound, job)
                    .await
                    .unwrap_or_else(|_| panic!("engine {engine}: the loop stopped on its signal"))
                    .unwrap();
                assert_eq!(
                    wake_with_status(),
                    WakeDispatchStatus::WorkerNotRunning,
                    "engine {engine}: its wake channel went with it"
                );
            }

            let job = tokio::spawn(start_cron_jobs(db.clone()));
            until_woken().await;
            request_shutdown();
            assert!(shutdown_requested());
            tokio::time::timeout(bound, job).await.unwrap().unwrap();
            let stop = CronStop::new();
            let job = tokio::spawn(run_cron_jobs(db.clone(), stop.clone()));
            until_woken().await;
            assert!(!shutdown_requested(), "a new loop starts unstopped");
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            assert!(
                !job.is_finished(),
                "an earlier request does not stop a later loop"
            );
            stop.stop();
            tokio::time::timeout(bound, job).await.unwrap().unwrap();

            // A loop called before a request and first polled after it is stopped by it.
            let called = start_cron_jobs(db.clone());
            request_shutdown();
            tokio::time::timeout(bound, called)
                .await
                .expect("a request made after the call stops the loop");
        });
        runtime.shutdown_timeout(std::time::Duration::from_secs(1));
        assert_eq!(
            Arc::strong_count(&db),
            1,
            "no loop kept the database handle"
        );
    }
}
