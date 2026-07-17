use crate::db::{Claim2551QSubmissionResult, Database};
use crate::forms::FilingStatus;
use crate::forms::form_2551q::Form2551QDraft;
use crate::profile::TaxpayerProfile;
use chrono::{Datelike, Utc};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

static WAKE_TX: OnceLock<mpsc::Sender<()>> = OnceLock::new();
static ACTIVE_JOBS: OnceLock<Arc<Mutex<HashSet<String>>>> = OnceLock::new();

pub fn get_active_jobs() -> Arc<Mutex<HashSet<String>>> {
    ACTIVE_JOBS
        .get_or_init(|| Arc::new(Mutex::new(HashSet::new())))
        .clone()
}

pub fn wake() {
    if let Some(tx) = WAKE_TX.get() {
        let _ = tx.try_send(());
    }
}

pub async fn start_cron_jobs(db: Arc<Mutex<Database>>) {
    info!("Background cron engine started");

    let (tx, mut rx) = mpsc::channel(1);
    let _ = WAKE_TX.set(tx);

    loop {
        // We run a tick, then wait for either 60 seconds or a wake signal.
        run_queue_tick(db.clone()).await;

        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
            _ = rx.recv() => {
                info!("Queue explicitly woken up by a wake signal.");
            }
        }
    }
}

/// Runs a single iteration of the background job queue.
/// Extracted from `start_cron_jobs` to allow deterministic unit testing
/// without being blocked by an infinite loop or thread sleep.
pub async fn run_queue_tick(db: Arc<Mutex<Database>>) {
    // Heartbeat log removed to prevent log spam. Job specific logs are emitted when jobs run.

    // Ensure we don't hold the DB lock across async network calls.
    // First, fetch profiles that have background cron enabled.
    let (profiles, global_cron_enabled) = {
        let db_guard = match db.lock() {
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
        crate::notification::send_notification(
            "BIR Vault Daemon",
            "Hello! The background cron is active.",
        );
    }

    if global_cron_enabled {
        for profile in profiles {
            // Task A: Form Submission Retries
            process_submission_queue(&profile, db.clone()).await;
        }
    }

    // Task C: Generic Job Queue (Custom Cron & One-off commands)
    process_generic_jobs(db.clone()).await;

    process_google_calendar_sync(db.clone()).await;

    // Signal the desktop app that the database was modified.
    // On macOS: instant via NSDistributedNotificationCenter.
    // On Linux/Windows: no-op (desktop uses PRAGMA data_version polling).
    crate::ipc::post_db_changed();
}

async fn process_google_calendar_sync(db: Arc<Mutex<Database>>) {
    const SIX_HOURS: i64 = 6 * 60 * 60;
    let should_sync = {
        let db = match db.lock() {
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

    let db_for_sync = db.clone();
    let results = match tokio::task::spawn_blocking(move || {
        crate::google_calendar::sync_all_profile_calendars(db_for_sync)
    })
    .await
    {
        Ok(results) => results,
        Err(error) => {
            warn!("Google Calendar background sync task failed: {error}");
            return;
        }
    };
    if results.is_empty() {
        if let Ok(db) = db.lock() {
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
                tin, report.inserted, report.updated, report.deleted
            ),
            Err(error) => warn!("Google Calendar sync failed for {}: {}", tin, error),
        }
    }
    let all_succeeded = results.iter().all(|(_, result)| result.is_ok());
    if let Ok(db) = db.lock() {
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
        errors: Vec<(String, String)>,
    },
    /// The row was canceled, replaced, or requeued after this job was spawned.
    Superseded,
}

fn queued_2551q_revision(draft: &Form2551QDraft) -> Option<Queued2551QRevision> {
    (matches!(draft.status, FilingStatus::Queued) && draft.submission_claim_token.is_none()).then(
        || Queued2551QRevision {
            fingerprint: draft.queued_submission_fingerprint.clone(),
            next_retry_at: draft.next_retry_at.clone(),
            submission_attempts: draft.submission_attempts,
        },
    )
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
            errors: vec![("profile_resolution".to_string(), error)],
        };
    }
    match draft.revalidate_queued_before_submission() {
        Ok(()) => Queued2551QPreparation::Ready { draft, revision },
        Err(errors) => Queued2551QPreparation::Rejected { draft, errors },
    }
}

fn validation_reason(errors: &[(String, String)]) -> String {
    errors
        .iter()
        .map(|(field, message)| format!("{field}: {message}"))
        .collect::<Vec<_>>()
        .join("; ")
}

async fn process_submission_queue(profile: &TaxpayerProfile, db: Arc<Mutex<Database>>) {
    let current_year = Utc::now().naive_utc().date().year() as u16;

    // We only retry current year forms to avoid excessive queries,
    // but we can query `list_draft_summaries` for the profile.
    let summaries = {
        let db_guard = match db.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        db_guard
            .list_draft_summaries(&profile.tin.full(), current_year)
            .unwrap_or_default()
    };

    for summary in summaries
        .into_iter()
        .filter(|s| s.status == FilingStatus::Queued)
        .filter(|s| crate::forms::can_queue_for_submission(&s.form_code))
    {
        let job_key = format!(
            "form:{}:{}:{}:{}",
            summary.form_code,
            profile.tin.full(),
            summary.taxable_year,
            summary.month.unwrap_or(summary.quarter.unwrap_or(0))
        );

        let active_jobs = get_active_jobs();
        {
            let mut jobs = match active_jobs.lock() {
                Ok(j) => j,
                Err(_) => continue,
            };
            if !jobs.insert(job_key.clone()) {
                continue; // Already running
            }
        }

        let profile_clone = profile.clone();
        let db_clone = db.clone();
        let active_jobs_clone = active_jobs.clone();

        tokio::spawn(async move {
            let _cleanup = JobCleanup {
                key: job_key,
                active_jobs: active_jobs_clone,
            };

            if summary.form_code == "2551Q" {
                let (loaded_draft, current_profile) = {
                    let db_guard = match db_clone.lock() {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                    let current_profile = match db_guard.get_profile(&profile_clone.tin.full()) {
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

                if loaded_draft.submission_claim_token.is_some() {
                    // A prior process may have crashed before or after BIR
                    // received the upload. Never lease-expire or retry this
                    // unknown outcome: doing so could create a duplicate filing.
                    warn!(
                        "Cron: Form {} has an unresolved submission outcome; automatic retry is disabled and support-assisted manual reconciliation is required",
                        loaded_draft.period_code()
                    );
                    crate::ipc::post_db_changed();
                    return;
                }

                let (mut draft, queue_revision) =
                    match prepare_queued_2551q(loaded_draft, &current_profile, None) {
                        Queued2551QPreparation::Ready { draft, revision } => (draft, revision),
                        Queued2551QPreparation::Rejected { draft, errors } => {
                            let reason = validation_reason(&errors);
                            warn!(
                                "Cron: Refusing invalid queued form {} before XML generation: {}",
                                draft.period_code(),
                                reason
                            );
                            if let Ok(db_guard) = db_clone.lock() {
                                let _ = db_guard.save_2551q_draft(&draft);
                            }
                            crate::ipc::post_db_changed();
                            return;
                        }
                        Queued2551QPreparation::Superseded => return,
                    };

                if let Some(next_retry) = &draft.next_retry_at
                    && let Ok(next_time) = chrono::DateTime::parse_from_rfc3339(next_retry)
                    && Utc::now() < next_time.with_timezone(&Utc)
                {
                    return;
                }

                info!(
                    "Cron: Attempting to submit queued form {} for {}",
                    draft.period_code(),
                    current_profile.tin.full()
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
                        if let Ok(db_guard) = db_clone.lock() {
                            let _ = db_guard.save_2551q_draft(&draft);
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
                        fail_draft_2551q(&mut draft, db_clone.clone(), e.to_string());
                        return;
                    }
                };

                // Resolve the exact transport identifier from the audited
                // capability registry at the irreversible boundary. Never
                // revive an uncertified submitter with a hard-coded fallback.
                let Some(form_type) = crate::forms::fileable_form_type_id("2551Q") else {
                    warn!(
                        "Cron: Refusing to submit {} because 2551Q is not authorized for queue submission",
                        draft.period_code()
                    );
                    return;
                };

                // Atomically claim the exact queue generation immediately
                // before the irreversible network boundary. Generic UI writes
                // reject this token, so cancel/requeue cannot win after claim.
                let claim_result = {
                    let db_guard = match db_clone.lock() {
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
                        // with the claimed row before the network call returns.
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
                        if let Ok(db_guard) = db_clone.lock() {
                            let _ = db_guard.save_2551q_draft(&rejected_draft);
                        }
                        crate::ipc::post_db_changed();
                        return;
                    }
                    Ok(Claim2551QSubmissionResult::Superseded) => {
                        info!(
                            "Cron: Submission job for {} was canceled or superseded before network I/O",
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

                match crate::transport::submit_iaf(form_type, &filename, &encrypted).await {
                    Ok(_) => {
                        info!("Cron: Successfully submitted queued form {}", filename);
                        let now = Utc::now();
                        crate::notification::send_notification(
                            "BIR Form Submitted",
                            &format!(
                                "Filename: {}\nTimestamp: {}",
                                filename,
                                now.format("%I:%M %p")
                            ),
                        );
                        draft.transition_to_submitted(filename.clone());
                        if let Ok(db_guard) = db_clone.lock() {
                            match db_guard.finish_claimed_2551q_submission(&draft, &claim_token) {
                                Ok(_) => schedule_email_poll(&current_profile, "2551Q", &db_guard),
                                Err(error) => warn!(
                                    "Cron: Form {} was transmitted but its submission claim could not be finalized: {}",
                                    draft.period_code(),
                                    error
                                ),
                            }
                        }
                    }
                    Err(error) => {
                        let error_category = match error {
                            crate::transport::TransportError::Ftp(_) => "ftp",
                            crate::transport::TransportError::Io(_) => "io",
                            crate::transport::TransportError::Rejected => "rejected",
                        };
                        // Once `submit_iaf` has started, an error does not prove
                        // that BIR received no bytes. Keep the durable claim just
                        // like a process crash: retrying could duplicate a return.
                        warn!(
                            error_category,
                            "Cron: Submission transport ended after the network claim; outcome is unknown and support-assisted manual reconciliation is required"
                        );
                        crate::ipc::post_db_changed();
                    }
                }
            }

            crate::ipc::post_db_changed();
        });
    }
}

pub fn schedule_email_poll(
    profile: &TaxpayerProfile,
    form_code: &str,
    db_guard: &std::sync::MutexGuard<Database>,
) {
    if profile.is_email_tracking_active() {
        let email = profile
            .imap_email
            .clone()
            .unwrap_or_else(|| profile.email.clone());
        let job_name = format!("Waiting for {} confirmation email for {}", form_code, email);
        let legacy_job_name = format!("Poll Receipts: {}", email);

        let jobs = db_guard.list_jobs().unwrap_or_default();
        let mut exists = false;
        for job in jobs.iter() {
            if (job.name == job_name || job.name == legacy_job_name)
                && job.status != "Archived"
                && job.status != "Done"
            {
                exists = true;
                break;
            }
        }

        if !exists {
            let new_job = crate::db::Job {
                id: None,
                name: job_name,
                job_type: "System".to_string(),
                cron_expr: Some("0 * * * * *".to_string()),
                command: Some(format!("bir_poll_email {}", email)),
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
}

fn fail_draft_2551q(
    draft: &mut crate::forms::form_2551q::Form2551QDraft,
    db: Arc<Mutex<Database>>,
    error_msg: String,
) {
    warn!(
        "Cron: Submission failed for {}: {}",
        draft.period_code(),
        error_msg
    );
    let attempts_before = draft.submission_attempts;
    draft.record_submission_failure(error_msg);

    if draft.submission_attempts >= 5 || attempts_before >= 4 {
        warn!(
            "Cron: Max attempts reached for {}. Giving up.",
            draft.period_code()
        );
    } else {
        let delay_mins = 2i64.pow(draft.submission_attempts - 1);
        info!("Cron: Next retry scheduled in {} mins", delay_mins);
    }

    if let Ok(db_guard) = db.lock() {
        let result = db_guard.save_2551q_draft(draft);
        if let Err(error) = result {
            warn!(
                "Cron: Failed to persist submission failure for {}: {}",
                draft.period_code(),
                error
            );
        }
    }
}

async fn process_generic_jobs(db: Arc<Mutex<Database>>) {
    let jobs = {
        let db_guard = match db.lock() {
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

        // Self-heal legacy bad cron strings
        if let Some(ref mut expr) = job.cron_expr
            && expr.trim() == "* * * * *"
        {
            *expr = "0 * * * * *".to_string();
            if job.status == "Failed" {
                job.status = "Queued".to_string();
            }
            if let Ok(db_guard) = db.lock() {
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
                            if let Ok(db_guard) = db.lock() {
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
        if let Ok(db_guard) = db.lock() {
            let _ = db_guard.save_job(job.clone());
        }

        let job_key = format!("job:{}", job.id.unwrap_or(0));
        let active_jobs = get_active_jobs();
        {
            let mut active = match active_jobs.lock() {
                Ok(a) => a,
                Err(_) => continue,
            };
            if !active.insert(job_key.clone()) {
                continue; // Already running
            }
        }

        let db_clone = db.clone();
        let active_jobs_clone = active_jobs.clone();

        tokio::spawn(async move {
            let _cleanup = JobCleanup {
                key: job_key,
                active_jobs: active_jobs_clone,
            };

            info!("Cron: Executing job '{}'", job.name);
            let mut success = true;

            if let Some(ref cmd) = job.command
                && !cmd.trim().is_empty()
            {
                if cmd.starts_with("bir_poll_email ") {
                    let email = cmd.trim_start_matches("bir_poll_email ").trim();
                    let (poll_success, still_pending, err_msg) =
                        crate::email::fetch_and_process_emails_for_address(email, db_clone.clone());
                    if poll_success {
                        let log = "Email polling completed successfully.".to_string();
                        info!(
                            "Cron: Email polling job '{}' completed successfully.",
                            job.name
                        );
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
                    }
                } else {
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
                    if let Ok(db_guard) = db_clone.lock() {
                        let _ = db_guard.save_job(job.clone());
                    }
                } else {
                    // Treated as one-off if empty string
                    if success {
                        if let Ok(db_guard) = db_clone.lock() {
                            let _ = db_guard.delete_job(job.id.unwrap());
                        }
                    } else {
                        job.retries += 1;
                        job.status = "Failed".to_string();
                        if let Ok(db_guard) = db_clone.lock() {
                            let _ = db_guard.save_job(job.clone());
                        }
                    }
                }
            } else {
                // One-off job
                if success {
                    if let Ok(db_guard) = db_clone.lock() {
                        let _ = db_guard.delete_job(job.id.unwrap());
                    }
                } else {
                    job.retries += 1;
                    job.status = "Failed".to_string();
                    if let Ok(db_guard) = db_clone.lock() {
                        let _ = db_guard.save_job(job.clone());
                    }
                }
            }

            crate::ipc::post_db_changed();
        });
    }
}

struct JobCleanup {
    key: String,
    active_jobs: Arc<Mutex<HashSet<String>>>,
}

impl Drop for JobCleanup {
    fn drop(&mut self) {
        if let Ok(mut jobs) = self.active_jobs.lock() {
            jobs.remove(&self.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::forms::form_2551q::Item13Election;
    use crate::profile::{EoptTier, IncomeTaxElection, TaxElectionHistory, TaxpayerProfile};
    use tempfile::NamedTempFile;

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
            "business_start_date": "2090-01-01"
        }))
        .unwrap()
    }

    fn reviewed_profile() -> TaxpayerProfile {
        let mut profile = test_profile();
        profile.tax_elections.push(TaxElectionHistory {
            taxable_year: 2099,
            election: IncomeTaxElection::GraduatedUnspecified,
            elected_at: chrono::NaiveDateTime::default(),
            source_form: "2551Qv2018".to_string(),
        });
        profile.ensure_profile_version_ledger();
        profile
    }

    fn queued_draft(profile: &TaxpayerProfile) -> Form2551QDraft {
        let mut draft = Form2551QDraft::new_from_effective_profile(profile, 2099, 1);
        draft.item_13_election = Item13Election::Graduated;
        draft
            .transition_to_queued()
            .expect("the reviewed draft should queue");
        draft
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
            Queued2551QPreparation::Rejected { draft, errors } => {
                assert_eq!(draft.status, FilingStatus::Draft);
                assert!(errors.iter().any(|(field, _)| field == "profile_snapshot"));
            }
            _ => panic!("a changed current profile must reject the queued return"),
        }
    }

    #[test]
    fn submission_preparation_rejects_an_unresolved_effective_profile() {
        let reviewed_profile = reviewed_profile();
        let draft = queued_draft(&reviewed_profile);
        let mut current_profile = reviewed_profile;
        current_profile.profile_versions.clear();

        match prepare_queued_2551q(draft, &current_profile, None) {
            Queued2551QPreparation::Rejected { draft, errors } => {
                assert_eq!(draft.status, FilingStatus::Draft);
                assert!(
                    draft
                        .last_error
                        .as_deref()
                        .is_some_and(|message| message.contains("effective taxpayer profile"))
                );
                assert!(errors.iter().any(|(field, message)| {
                    field == "profile_resolution" && message.contains("No confirmed")
                }));
            }
            _ => panic!("an unresolved effective profile must reject the queued return"),
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
}
