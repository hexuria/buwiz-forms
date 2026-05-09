use crate::db::Database;
use crate::forms::FilingStatus;
use crate::profile::TaxpayerProfile;
use chrono::{Datelike, Utc};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

pub async fn start_cron_jobs(db: Arc<Mutex<Database>>) {
    info!("Background cron engine started");

    loop {
        // Run every 1 minute
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        run_queue_tick(db.clone()).await;
    }
}

/// Runs a single iteration of the background job queue.
/// Extracted from `start_cron_jobs` to allow deterministic unit testing
/// without being blocked by an infinite loop or thread sleep.
pub async fn run_queue_tick(db: Arc<Mutex<Database>>) {
    info!("Running background cron jobs...");

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

    // Signal the desktop app that the database was modified.
    // On macOS: instant via NSDistributedNotificationCenter.
    // On Linux/Windows: no-op (desktop uses PRAGMA data_version polling).
    crate::ipc::post_db_changed();
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
    {
        if summary.form_code == "2551Q" {
            let mut draft = {
                let db_guard = match db.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                match db_guard.get_2551q_draft(
                    &profile.tin.full(),
                    summary.taxable_year,
                    summary.quarter.unwrap_or(0),
                ) {
                    Ok(Some(d)) => d,
                    _ => continue,
                }
            };

            if let Some(next_retry) = &draft.next_retry_at
                && let Ok(next_time) = chrono::DateTime::parse_from_rfc3339(next_retry)
                && Utc::now() < next_time.with_timezone(&Utc)
            {
                continue;
            }

            info!(
                "Cron: Attempting to submit queued form {} for {}",
                draft.period_code(),
                profile.tin.full()
            );

            let form_type = "2551Qv2018";
            let filename = draft.default_submission_filename();
            let xml_payload = draft.to_bir_xml_payload();
            let encrypted = match crate::crypto::compress_and_encrypt(
                xml_payload.as_bytes(),
                crate::crypto::BIR_IAF_PASSPHRASE,
            ) {
                Ok(enc) => enc,
                Err(e) => {
                    fail_draft_2551q(&mut draft, db.clone(), e.to_string());
                    continue;
                }
            };

            match crate::transport::submit_iaf(form_type, &filename, &encrypted).await {
                Ok(_) => {
                    info!("Cron: Successfully submitted queued form {}", filename);
                    let now = Utc::now();
                    crate::notification::send_notification(
                        "BIR Form Submitted",
                        &format!("Filename: {}\nTimestamp: {}", filename, now.format("%I:%M %p")),
                    );
                    draft.transition_to_submitted(filename.clone());
                    if let Ok(db_guard) = db.lock() {
                        let _ = db_guard.save_2551q_draft(&draft);
                        schedule_email_poll(profile, "2551Q", &db_guard);
                    }
                }
                Err(e) => fail_draft_2551q(&mut draft, db.clone(), e.to_string()),
            }
        } else if summary.form_code == "1601C" {
            let mut draft = {
                let db_guard = match db.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                match db_guard.get_1601c_draft(
                    &profile.tin.full(),
                    summary.taxable_year,
                    summary.month.unwrap_or(0),
                ) {
                    Ok(Some(d)) => d,
                    _ => continue,
                }
            };

            if let Some(next_retry) = &draft.next_retry_at
                && let Ok(next_time) = chrono::DateTime::parse_from_rfc3339(next_retry)
                && Utc::now() < next_time.with_timezone(&Utc)
            {
                continue;
            }

            info!(
                "Cron: Attempting to submit queued form {} for {}",
                draft.period_code(),
                profile.tin.full()
            );

            let form_type = "1601Cv2018";
            let filename = draft.default_submission_filename();
            let xml_payload = draft.to_bir_xml_payload();
            let encrypted = match crate::crypto::compress_and_encrypt(
                xml_payload.as_bytes(),
                crate::crypto::BIR_IAF_PASSPHRASE,
            ) {
                Ok(enc) => enc,
                Err(e) => {
                    fail_draft_1601c(&mut draft, db.clone(), e.to_string());
                    continue;
                }
            };

            match crate::transport::submit_iaf(form_type, &filename, &encrypted).await {
                Ok(_) => {
                    info!("Cron: Successfully submitted queued form {}", filename);
                    let now = Utc::now();
                    crate::notification::send_notification(
                        "BIR Form Submitted",
                        &format!("Filename: {}\nTimestamp: {}", filename, now.format("%I:%M %p")),
                    );
                    draft.transition_to_submitted(filename.clone());
                    if let Ok(db_guard) = db.lock() {
                        let _ = db_guard.save_form_draft(
                            &draft.tin,
                            "1601C",
                            draft.taxable_year,
                            Some(draft.month),
                            &draft.status,
                            &draft
                        );
                        schedule_email_poll(profile, "1601C", &db_guard);
                    }
                }
                Err(e) => fail_draft_1601c(&mut draft, db.clone(), e.to_string()),
            }
        }
    }
}

fn schedule_email_poll(profile: &TaxpayerProfile, form_code: &str, db_guard: &std::sync::MutexGuard<Database>) {
    if profile.is_email_tracking_active() {
        let email = profile.imap_email.clone().unwrap_or_else(|| profile.email.clone());
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
    warn!("Cron: Submission failed for {}: {}", draft.period_code(), error_msg);
    let attempts_before = draft.submission_attempts;
    draft.record_submission_failure(error_msg);

    if draft.submission_attempts >= 5 || attempts_before >= 4 {
        warn!("Cron: Max attempts reached for {}. Giving up.", draft.period_code());
    } else {
        let delay_mins = 2i64.pow(draft.submission_attempts - 1);
        info!("Cron: Next retry scheduled in {} mins", delay_mins);
    }

    if let Ok(db_guard) = db.lock() {
        let _ = db_guard.save_2551q_draft(draft);
    }
}

fn fail_draft_1601c(
    draft: &mut crate::forms::form_1601c::Form1601CDraft,
    db: Arc<Mutex<Database>>,
    error_msg: String,
) {
    warn!("Cron: Submission failed for {}: {}", draft.period_code(), error_msg);
    let attempts_before = draft.submission_attempts;
    draft.record_submission_failure(error_msg);

    if draft.submission_attempts >= 5 || attempts_before >= 4 {
        warn!("Cron: Max attempts reached for {}. Giving up.", draft.period_code());
    } else {
        let delay_mins = 2i64.pow(draft.submission_attempts - 1);
        info!("Cron: Next retry scheduled in {} mins", delay_mins);
    }

    if let Ok(db_guard) = db.lock() {
        let _ = db_guard.save_form_draft(
            &draft.tin,
            "1601C",
            draft.taxable_year,
            Some(draft.month),
            &draft.status,
            draft
        );
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

        info!("Cron: Executing job '{}'", job.name);
        let mut success = true;

        if let Some(ref cmd) = job.command
            && !cmd.trim().is_empty()
        {
            if cmd.starts_with("bir_poll_email ") {
                let email = cmd.trim_start_matches("bir_poll_email ").trim();
                let (poll_success, still_pending, err_msg) =
                    crate::email::fetch_and_process_emails_for_address(email, db.clone());
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
                if let Ok(db_guard) = db.lock() {
                    let _ = db_guard.save_job(job.clone());
                }
            } else {
                // Treated as one-off if empty string
                if success {
                    if let Ok(db_guard) = db.lock() {
                        let _ = db_guard.delete_job(job.id.unwrap());
                    }
                } else {
                    job.retries += 1;
                    job.status = "Failed".to_string();
                    if let Ok(db_guard) = db.lock() {
                        let _ = db_guard.save_job(job.clone());
                    }
                }
            }
        } else {
            // One-off job
            if success {
                if let Ok(db_guard) = db.lock() {
                    let _ = db_guard.delete_job(job.id.unwrap());
                }
            } else {
                job.retries += 1;
                job.status = "Failed".to_string();
                if let Ok(db_guard) = db.lock() {
                    let _ = db_guard.save_job(job.clone());
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use tempfile::NamedTempFile;

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
}
